"""
Adaptador DXF (ezdxf) - CRAFT ELE
Camada de infraestrutura e integração com CAD (CADCraft / AutoCAD / LibreCAD).
Responsável por leitura, escrita, extração de atributos e correlação topológica.
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

import ezdxf
from ezdxf.document import Drawing
from ezdxf.entities import Insert, Line, LWPolyline, Attrib, AttDef, Text, MText


@dataclass
class AttributeInfo:
    """Metadados de um atributo de bloco (ATTRIB ou ATTDEF)."""
    tag: str
    value: str
    flags: int
    is_invisible: bool
    is_constant: bool
    source_type: str  # 'ATTRIB' (vinculado ao INSERT) ou 'ATTDEF'/'TEXT' (fallback na definição)


@dataclass
class BlockInsertInfo:
    """Informações de um bloco inserido (INSERT) no Modelspace."""
    handle: str
    name: str
    layer: str
    position: Tuple[float, float, float]
    rotation: float
    attribs: List[AttributeInfo] = field(default_factory=list)
    has_attached_attribs: bool = False
    definition_fallback_entities: List[str] = field(default_factory=list)

    @property
    def x(self) -> float:
        return self.position[0]

    @property
    def y(self) -> float:
        return self.position[1]

    @property
    def z(self) -> float:
        return self.position[2]

    def get_attr_value(self, tag: str, default: Optional[str] = None) -> Optional[str]:
        for attr in self.attribs:
            if attr.tag.upper() == tag.upper():
                return attr.value
        return default

    def get_first_attr(self, tags: List[str], default: Optional[str] = None) -> Optional[str]:
        """Busca o primeiro atributo coincidente em uma lista de sinônimos."""
        for t in tags:
            val = self.get_attr_value(t)
            if val is not None and str(val).strip() != "":
                return str(val).strip()
        return default

    @property
    def tipo(self) -> str:
        """Tipo/categoria do elemento elétrico."""
        val = self.get_first_attr(["TIPO", "TIPO_PONTO", "TYPE", "CATEGORIA"])
        if val:
            return val
        # Inferência por nome do bloco caso atributo não esteja explícito
        n = self.name.upper()
        if "INT" in n or "SWITCH" in n:
            return "INTERRUPTOR"
        if "TUG" in n or "TOMADA" in n or "TOM" in n:
            return "TOMADA"
        if "TUE" in n:
            return "TOMADA_TUE"
        if "LUM" in n or "LAMP" in n or "ILUM" in n:
            return "ILUMINACAO"
        if "QD" in n or "QUADRO" in n:
            return "QUADRO_DISTRIBUICAO"
        return "OUTRO"

    @property
    def potencia_raw(self) -> Optional[str]:
        """Texto bruto do atributo de potência (ex: '100VA', '600W', '100W')."""
        return self.get_first_attr(["POTENCIA", "POT", "POWER", "W", "VA"])

    @property
    def potencia_num(self) -> Optional[float]:
        """Valor numérico da potência (se aplicável)."""
        raw = self.potencia_raw
        if not raw:
            return None
        import re
        m = re.search(r"[-+]?[0-9]*\.?[0-9]+", raw.replace(",", "."))
        if m:
            try:
                return float(m.group(0))
            except ValueError:
                return None
        return None

    @property
    def tensao(self) -> str:
        """Tensão de operação (ex: '127V', '220V', '220/127V')."""
        return self.get_first_attr(["TENSAO", "VOLTS", "VOLTAGEM", "TENSÃO"], default="")

    @property
    def fases(self) -> str:
        """Número de fases (1, 2, 3 ou 'MONO', 'BI', 'TRI')."""
        return self.get_first_attr(["FASES", "NUMERO_FASES", "NUM_FASES", "FASE", "F"], default="")


    @property
    def circuito(self) -> str:
        """Circuito elétrico ao qual o bloco pertence."""
        return self.get_first_attr(["CIRCUITO", "CIRC", "CIRCUIT"], default="")

    @property
    def comando(self) -> str:
        """Comando de acionamento (apenas para interruptores/pulsadores)."""
        return self.get_first_attr(["COMANDO", "CMD"], default="")

    @property
    def tag(self) -> str:
        """Identificador individual do ponto."""
        return self.get_first_attr(["TAG", "ID", "NOME"], default=self.name)

    @property
    def is_switch(self) -> bool:
        """Verifica se o bloco é um interruptor/comando."""
        t = self.tipo.upper()
        return "INT" in t or "SWITCH" in t or bool(self.comando)

    @property
    def is_board(self) -> bool:
        """Verifica se o bloco é um Quadro de Distribuição."""
        t = self.tipo.upper()
        return "QUADRO" in t or "QD" in t

    @property
    def is_load(self) -> bool:
        """Verifica se o bloco é uma carga (tomada, iluminação, equipamento)."""
        return not self.is_switch and not self.is_board

    def validate_schema(self) -> List[str]:
        """
        Valida a consistência dos atributos segundo as regras de negócio:
        - Interruptores: DEVEM ter comando, NÃO DEVEM ter potência.
        - Cargas: DEVEM ter potência, tensão, fases e tipo.
        """
        warnings: List[str] = []

        if self.is_switch:
            if not self.comando:
                warnings.append("Interruptor sem atributo 'COMANDO' (letra de acionamento).")
            if self.potencia_num is not None and self.potencia_num > 0:
                warnings.append(f"Atenção: Interruptor deve ter potência 0W (atual: '{self.potencia_raw}').")
        elif self.is_board:
            if not self.tensao:
                warnings.append("Quadro sem atributo 'TENSAO' definida.")
        else:
            # Ponto de Carga (Tomadas, Iluminação, etc.)
            if self.potencia_num is None:
                warnings.append("Ponto de carga sem atributo 'POTENCIA'.")
            if not self.tensao:
                warnings.append("Ponto sem atributo 'TENSAO'.")
            if not self.fases:
                warnings.append("Ponto sem atributo 'NUMERO_FASES' ou 'FASES'.")


        return warnings


@dataclass
class ConduitEndpointMatch:
    """Associação de extremidade de eletroduto a um bloco por proximidade espacial."""
    block: Optional[BlockInsertInfo]
    distance: float
    point: Tuple[float, float]


@dataclass
class ConduitSegmentInfo:
    """Segmento de eletroduto representado por LINE ou LWPOLYLINE."""
    handle: str
    entity_type: str
    layer: str
    start_point: Tuple[float, float]
    end_point: Tuple[float, float]
    length_2d: float
    start_match: Optional[ConduitEndpointMatch] = None
    end_match: Optional[ConduitEndpointMatch] = None

    @property
    def is_fully_connected(self) -> bool:
        return bool(self.start_match and self.start_match.block and self.end_match and self.end_match.block)

    @property
    def is_dangling(self) -> bool:
        has_start = bool(self.start_match and self.start_match.block)
        has_end = bool(self.end_match and self.end_match.block)
        return (has_start and not has_end) or (not has_start and has_end)

    @property
    def is_isolated(self) -> bool:
        return not bool(self.start_match and self.start_match.block) and not bool(self.end_match and self.end_match.block)


@dataclass
class DXFDiagnosticReport:
    """Relatório estruturado de diagnóstico do arquivo DXF."""
    filename: str
    dxf_version: str
    total_blocks_defined: int
    total_inserts: int
    total_lines: int
    total_lwpolylines: int
    inserts: List[BlockInsertInfo]
    conduits: List[ConduitSegmentInfo]
    tolerance_used: float
    attribute_mode_detected: str  # 'NATIVE_ATTRIB', 'BLOCK_DEF_FALLBACK', 'MIXED', 'NONE'


class CADCraftDXFAdapter:
    """
    Adaptador de alto nível para manipular arquivos DXF gerados pelo CADCraft ou outros softwares CAD.
    """

    DEFAULT_CAPTURE_RADIUS: float = 0.20  # Tolerância espacial padrão: 20cm ou 0.20 unidades

    @staticmethod
    def distance_2d(p1: Tuple[float, float], p2: Tuple[float, float]) -> float:
        """Calcula a distância euclidiana 2D entre dois pontos."""
        return math.hypot(p1[0] - p2[0], p1[1] - p2[1])

    @classmethod
    def load_document(cls, filepath: str | Path) -> Drawing:
        """Carrega um arquivo DXF usando ezdxf."""
        path = Path(filepath)
        if not path.is_file():
            raise FileNotFoundError(f"Arquivo DXF não encontrado: {path.resolve()}")
        return ezdxf.readfile(str(path))

    @classmethod
    def extract_block_inserts(cls, doc: Drawing) -> List[BlockInsertInfo]:
        """
        Extrai todos os INSERTs do Modelspace, analisando atributos vinculados
        e realizando inspeção profunda (fallback) na definição do bloco caso necessário.
        """
        msp = doc.modelspace()
        inserts_info: List[BlockInsertInfo] = []

        for entity in msp.query("INSERT"):
            insert: Insert = entity
            handle = insert.dxf.handle
            name = insert.dxf.name
            layer = insert.dxf.layer
            pos = (float(insert.dxf.insert.x), float(insert.dxf.insert.y), float(insert.dxf.insert.z))
            rotation = float(getattr(insert.dxf, 'rotation', 0.0))

            attrib_list: List[AttributeInfo] = []
            has_attached = False

            # 1. Tentar ler atributos nativos vinculados à inserção (insert.attribs)
            raw_attribs = list(insert.attribs)
            if raw_attribs:
                has_attached = True
                for att in raw_attribs:
                    flags = int(getattr(att.dxf, 'flags', 0))
                    is_invisible = bool(flags & 1)
                    is_constant = bool(flags & 2)
                    attrib_list.append(
                        AttributeInfo(
                            tag=str(att.dxf.tag),
                            value=str(att.dxf.text),
                            flags=flags,
                            is_invisible=is_invisible,
                            is_constant=is_constant,
                            source_type="ATTRIB",
                        )
                    )

            # 2. Inspeção profunda (Fallback): se insert.attribs estiver vazio,
            # verificar na definição do bloco (doc.blocks[name])
            fallback_entities_desc: List[str] = []
            if not has_attached and name in doc.blocks:
                block_def = doc.blocks[name]
                for b_entity in block_def:
                    b_type = b_entity.dxftype()
                    if b_type == "ATTDEF":
                        attdef: AttDef = b_entity
                        flags = int(getattr(attdef.dxf, 'flags', 0))
                        is_invisible = bool(flags & 1)
                        is_constant = bool(flags & 2)
                        tag = str(attdef.dxf.tag)
                        val = str(getattr(attdef.dxf, 'text', ''))
                        attrib_list.append(
                            AttributeInfo(
                                tag=tag,
                                value=val,
                                flags=flags,
                                is_invisible=is_invisible,
                                is_constant=is_constant,
                                source_type="ATTDEF (Bloco Def)",
                            )
                        )
                        fallback_entities_desc.append(f"ATTDEF:{tag}={val}")
                    elif b_type in ("TEXT", "MTEXT"):
                        text_val = str(getattr(b_entity.dxf, 'text', ''))
                        attrib_list.append(
                            AttributeInfo(
                                tag=f"_STATIC_{b_type}",
                                value=text_val,
                                flags=0,
                                is_invisible=False,
                                is_constant=True,
                                source_type=f"{b_type} (Bloco Def)",
                            )
                        )
                        fallback_entities_desc.append(f"{b_type}:{text_val}")

            inserts_info.append(
                BlockInsertInfo(
                    handle=handle,
                    name=name,
                    layer=layer,
                    position=pos,
                    rotation=rotation,
                    attribs=attrib_list,
                    has_attached_attribs=has_attached,
                    definition_fallback_entities=fallback_entities_desc,
                )
            )

        return inserts_info

    @classmethod
    def extract_conduit_segments(cls, doc: Drawing, layers: Optional[List[str]] = None) -> List[ConduitSegmentInfo]:
        """
        Extrai entidades de fiação/eletroduto (LINE e LWPOLYLINE) do Modelspace.
        Se layers for especificado, filtra pelas camadas dadas.
        """
        msp = doc.modelspace()
        segments: List[ConduitSegmentInfo] = []

        # Processar LINEs
        for line in msp.query("LINE"):
            layer = line.dxf.layer
            if layers and layer.upper() not in [l.upper() for l in layers]:
                continue

            p_start = (float(line.dxf.start.x), float(line.dxf.start.y))
            p_end = (float(line.dxf.end.x), float(line.dxf.end.y))
            length = cls.distance_2d(p_start, p_end)

            segments.append(
                ConduitSegmentInfo(
                    handle=line.dxf.handle,
                    entity_type="LINE",
                    layer=layer,
                    start_point=p_start,
                    end_point=p_end,
                    length_2d=length,
                )
            )

        # Processar LWPOLYLINEs
        for poly in msp.query("LWPOLYLINE"):
            layer = poly.dxf.layer
            if layers and layer.upper() not in [l.upper() for l in layers]:
                continue

            pts = list(poly.get_points(format="xy"))
            if len(pts) < 2:
                continue

            p_start = (float(pts[0][0]), float(pts[0][1]))
            p_end = (float(pts[-1][0]), float(pts[-1][1]))

            # Comprimento 2D total acumulado da polilinha
            total_len = 0.0
            for i in range(len(pts) - 1):
                total_len += cls.distance_2d((pts[i][0], pts[i][1]), (pts[i+1][0], pts[i+1][1]))

            segments.append(
                ConduitSegmentInfo(
                    handle=poly.dxf.handle,
                    entity_type="LWPOLYLINE",
                    layer=layer,
                    start_point=p_start,
                    end_point=p_end,
                    length_2d=total_len,
                )
            )

        return segments

    @classmethod
    def correlate_connectivity(
        cls,
        blocks: List[BlockInsertInfo],
        conduits: List[ConduitSegmentInfo],
        tolerance: float = DEFAULT_CAPTURE_RADIUS,
    ) -> None:
        """
        Conecta geometricamente as extremidades dos eletrodutos aos blocos elétricos
        utilizando uma tolerância espacial (raio de captura em unidades de desenho).
        Modifica in-place os objetos ConduitSegmentInfo.
        """
        for cond in conduits:
            # Buscar bloco mais próximo para start_point
            best_start_block: Optional[BlockInsertInfo] = None
            min_dist_start = float("inf")
            for b in blocks:
                d = cls.distance_2d(cond.start_point, (b.x, b.y))
                if d <= tolerance and d < min_dist_start:
                    min_dist_start = d
                    best_start_block = b

            cond.start_match = ConduitEndpointMatch(
                block=best_start_block,
                distance=min_dist_start if best_start_block else float("inf"),
                point=cond.start_point,
            )

            # Buscar bloco mais próximo para end_point
            best_end_block: Optional[BlockInsertInfo] = None
            min_dist_end = float("inf")
            for b in blocks:
                d = cls.distance_2d(cond.end_point, (b.x, b.y))
                if d <= tolerance and d < min_dist_end:
                    min_dist_end = d
                    best_end_block = b

            cond.end_match = ConduitEndpointMatch(
                block=best_end_block,
                distance=min_dist_end if best_end_block else float("inf"),
                point=cond.end_point,
            )

    @classmethod
    def run_diagnostics(
        cls,
        doc: Drawing,
        filename: str = "memoria.dxf",
        tolerance: float = DEFAULT_CAPTURE_RADIUS,
    ) -> DXFDiagnosticReport:
        """Executa a rotina completa de diagnóstico em um documento DXF."""
        blocks = cls.extract_block_inserts(doc)
        conduits = cls.extract_conduit_segments(doc)
        cls.correlate_connectivity(blocks, conduits, tolerance=tolerance)

        # Analisar o modo de atributos detectado
        has_native = any(b.has_attached_attribs for b in blocks)
        has_fallback = any(not b.has_attached_attribs and len(b.attribs) > 0 for b in blocks)
        has_none = any(len(b.attribs) == 0 for b in blocks)

        if has_native and not has_fallback:
            attr_mode = "NATIVE_ATTRIB"
        elif has_fallback and not has_native:
            attr_mode = "BLOCK_DEF_FALLBACK"
        elif has_native and has_fallback:
            attr_mode = "MIXED"
        else:
            attr_mode = "NONE"

        line_count = sum(1 for c in conduits if c.entity_type == "LINE")
        poly_count = sum(1 for c in conduits if c.entity_type == "LWPOLYLINE")

        return DXFDiagnosticReport(
            filename=filename,
            dxf_version=doc.dxfversion,
            total_blocks_defined=len(doc.blocks),
            total_inserts=len(blocks),
            total_lines=line_count,
            total_lwpolylines=poly_count,
            inserts=blocks,
            conduits=conduits,
            tolerance_used=tolerance,
            attribute_mode_detected=attr_mode,
        )

    @classmethod
    def create_synthetic_dxf(cls, output_path: str | Path) -> Path:
        """
        Cria um arquivo DXF sintético completo para teste e PoC:
        - Blocos com ATTRIB reais vinculados ao INSERT
        - Bloco com ATTDEF/TEXT interno para teste de fallback
        - Eletrodutos (LINE e LWPOLYLINE) com conexões exatas e com pequeno desvio (para testar raio de captura)
        - Eletroduto com ponta solta (dangling) e trecho desconectado (isolated)
        """
        path = Path(output_path)
        path.parent.mkdir(parents=True, exist_ok=True)

        doc = ezdxf.new("R2010", setup=True)
        msp = doc.modelspace()

        # Configurar Camadas
        doc.layers.add("ELETRO_PONTOS", color=1)       # Vermelho
        doc.layers.add("ELETRODUTO", color=4)          # Ciano
        doc.layers.add("ELETRO_QUADROS", color=2)      # Amarelo
        doc.layers.add("ELETRO_COMANDOS", color=3)     # Verde

        # -------------------------------------------------------------
        # 1. DEFINIÇÃO DE BLOCOS
        # -------------------------------------------------------------

        # Bloco A: Quadro de Distribuição (QD)
        b_qd = doc.blocks.new(name="QUADRO_DISTRIBUICAO")
        b_qd.add_lwpolyline([(-0.3, -0.1), (0.3, -0.1), (0.3, 0.1), (-0.3, 0.1)], close=True)
        b_qd.add_attdef(tag="TAG", text="QD-01", dxfattribs={"prompt": "Identificação do Quadro", "flags": 0})
        b_qd.add_attdef(tag="POTENCIA", text="12000W", dxfattribs={"flags": 0})
        b_qd.add_attdef(tag="TENSAO", text="220/127V", dxfattribs={"flags": 0})
        b_qd.add_attdef(tag="DISJUNTOR_GERAL", text="50A", dxfattribs={"flags": 1})  # Flag 1 = Invisível

        # Bloco B: Luminária de Teto (Ponto de Iluminação)
        b_lum = doc.blocks.new(name="LUMINARIA_TETO")
        b_lum.add_circle((0.0, 0.0), radius=0.25)
        b_lum.add_attdef(tag="TAG", text="ILUM", dxfattribs={"flags": 0})
        b_lum.add_attdef(tag="CIRCUITO", text="1", dxfattribs={"flags": 0})
        b_lum.add_attdef(tag="POTENCIA", text="100VA", dxfattribs={"flags": 0})
        b_lum.add_attdef(tag="COMANDO", text="a", dxfattribs={"flags": 0})

        # Bloco C: Interruptor Simples
        b_int = doc.blocks.new(name="INTERRUPTOR_SIMPLES")
        b_int.add_circle((0.0, 0.0), radius=0.15)
        b_int.add_line((0.0, -0.15), (0.0, 0.15))
        b_int.add_attdef(tag="TAG", text="INT-1", dxfattribs={"flags": 0})
        b_int.add_attdef(tag="COMANDO", text="a", dxfattribs={"flags": 0})
        b_int.add_attdef(tag="CIRCUITO", text="1", dxfattribs={"flags": 0})

        # Bloco D: Tomada Baixa (TUG)
        b_tug = doc.blocks.new(name="TOMADA_TUG_BAIXA")
        b_tug.add_arc((0.0, 0.0), radius=0.2, start_angle=0, end_angle=180)
        b_tug.add_attdef(tag="TAG", text="TUG-1", dxfattribs={"flags": 0})
        b_tug.add_attdef(tag="CIRCUITO", text="2", dxfattribs={"flags": 0})
        b_tug.add_attdef(tag="POTENCIA", text="100VA", dxfattribs={"flags": 0})

        # Bloco E: Caixa de Passagem (SEM ATTRIBs instanciados - Fallback para ATTDEF e TEXT)
        b_cp = doc.blocks.new(name="CAIXA_PASSAGEM_FALLBACK")
        b_cp.add_lwpolyline([(-0.15, -0.15), (0.15, -0.15), (0.15, 0.15), (-0.15, 0.15)], close=True)
        b_cp.add_attdef(tag="TAG_FALLBACK", text="CP-01", dxfattribs={"prompt": "Tag da Caixa", "flags": 0})
        b_cp.add_text("CP_4x4", dxfattribs={"height": 0.1, "insert": (-0.1, -0.05)})

        # -------------------------------------------------------------
        # 2. INSERÇÕES (INSERTS) NO MODELSPACE
        # -------------------------------------------------------------

        # Inserção 1: QD em (0.0, 0.0, 1.5) com ATTRIBs explícitos
        ins_qd = msp.add_blockref("QUADRO_DISTRIBUICAO", (0.0, 0.0, 1.5), dxfattribs={"layer": "ELETRO_QUADROS"})
        ins_qd.add_attrib("TAG", "QD-01", dxfattribs={"flags": 0})
        ins_qd.add_attrib("TIPO", "QUADRO", dxfattribs={"flags": 0})
        ins_qd.add_attrib("POTENCIA", "15400W", dxfattribs={"flags": 0})
        ins_qd.add_attrib("TENSAO", "220/127V", dxfattribs={"flags": 0})
        ins_qd.add_attrib("FASES", "2", dxfattribs={"flags": 0})
        ins_qd.add_attrib("DISJUNTOR_GERAL", "63A", dxfattribs={"flags": 1})  # Invisível (código 70 = 1)

        # Inserção 2: Luminária em (4.0, 0.0, 2.8) com ATTRIBs explícitos
        ins_lum = msp.add_blockref("LUMINARIA_TETO", (4.0, 0.0, 2.8), dxfattribs={"layer": "ELETRO_PONTOS"})
        ins_lum.add_attrib("TAG", "LUM-SALA", dxfattribs={"flags": 0})
        ins_lum.add_attrib("TIPO", "ILUMINACAO", dxfattribs={"flags": 0})
        ins_lum.add_attrib("CIRCUITO", "1", dxfattribs={"flags": 0})
        ins_lum.add_attrib("POTENCIA", "100VA", dxfattribs={"flags": 0})
        ins_lum.add_attrib("TENSAO", "127V", dxfattribs={"flags": 0})
        ins_lum.add_attrib("FASES", "1", dxfattribs={"flags": 0})
        ins_lum.add_attrib("COMANDO", "a", dxfattribs={"flags": 0})

        # Inserção 3: Interruptor em (4.0, 3.0, 1.2) com ATTRIBs explícitos (POTÊNCIA ZERADA: 0W)
        ins_int = msp.add_blockref("INTERRUPTOR_SIMPLES", (4.0, 3.0, 1.2), dxfattribs={"layer": "ELETRO_COMANDOS"})
        ins_int.add_attrib("TAG", "INT-SALA", dxfattribs={"flags": 0})
        ins_int.add_attrib("TIPO", "INTERRUPTOR", dxfattribs={"flags": 0})
        ins_int.add_attrib("POTENCIA", "0W", dxfattribs={"flags": 0})
        ins_int.add_attrib("COMANDO", "a", dxfattribs={"flags": 0})
        ins_int.add_attrib("CIRCUITO", "1", dxfattribs={"flags": 0})
        ins_int.add_attrib("TENSAO", "127V", dxfattribs={"flags": 0})
        ins_int.add_attrib("FASES", "1", dxfattribs={"flags": 0})


        # Inserção 4: Tomada em (0.0, 3.0, 0.3) com ATTRIBs explícitos
        ins_tug = msp.add_blockref("TOMADA_TUG_BAIXA", (0.0, 3.0, 0.3), dxfattribs={"layer": "ELETRO_PONTOS"})
        ins_tug.add_attrib("TAG", "TUG-01", dxfattribs={"flags": 0})
        ins_tug.add_attrib("TIPO", "TUG", dxfattribs={"flags": 0})
        ins_tug.add_attrib("CIRCUITO", "2", dxfattribs={"flags": 0})
        ins_tug.add_attrib("POTENCIA", "600VA", dxfattribs={"flags": 0})
        ins_tug.add_attrib("TENSAO", "127V", dxfattribs={"flags": 0})
        ins_tug.add_attrib("FASES", "1", dxfattribs={"flags": 0})

        # Inserção 5: Caixa de Passagem em (2.0, 1.5, 0.0) SEM ATTRIBs (para testar Fallback)
        msp.add_blockref("CAIXA_PASSAGEM_FALLBACK", (2.0, 1.5, 0.0), dxfattribs={"layer": "ELETRO_PONTOS"})

        # -------------------------------------------------------------
        # 3. TUBULAÇÃO / ELETRODUTOS (LINE e LWPOLYLINE)
        # Nota: extremidades com pequenos deslocamentos para testar raio de captura
        # -------------------------------------------------------------

        # Trecho 1 (LINE): QD -> Luminária (com desvio de 0.03m e 0.05m, tolerância de 0.20m captura)
        msp.add_line(
            start=(0.03, 0.02),
            end=(3.95, -0.01),
            dxfattribs={"layer": "ELETRODUTO"}
        )

        # Trecho 2 (LWPOLYLINE): Luminária -> Interruptor (via ponto intermediário de teto/parede)
        msp.add_lwpolyline(
            points=[(4.02, 0.04), (4.0, 1.5), (3.98, 2.97)],
            dxfattribs={"layer": "ELETRODUTO"}
        )

        # Trecho 3 (LINE): QD -> Tomada TUG
        msp.add_line(
            start=(-0.02, 0.05),
            end=(0.01, 2.96),
            dxfattribs={"layer": "ELETRODUTO"}
        )

        # Trecho 4 (LINE): Luminária -> Caixa de Passagem Fallback
        msp.add_line(
            start=(3.96, 0.03),
            end=(2.03, 1.48),
            dxfattribs={"layer": "ELETRODUTO"}
        )

        # Trecho 5 (LINE - Ponta Solta): Caixa de Passagem -> Ponto desconectado (2.0, 4.5)
        msp.add_line(
            start=(1.98, 1.52),
            end=(2.0, 4.5),
            dxfattribs={"layer": "ELETRODUTO"}
        )

        # Trecho 6 (LINE - Isolado): Trecho solto sem blocos perto
        msp.add_line(
            start=(10.0, 10.0),
            end=(12.0, 10.0),
            dxfattribs={"layer": "ELETRODUTO"}
        )

        doc.saveas(str(path))
        return path
