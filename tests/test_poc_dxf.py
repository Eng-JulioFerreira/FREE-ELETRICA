"""
Testes unitários e de integração para a PoC de leitura DXF (CRAFT ELE / CADCraft).
"""

import subprocess
import sys
from pathlib import Path
import pytest
import ezdxf

from src.adapters.dxf_adapter import CADCraftDXFAdapter, BlockInsertInfo, ConduitSegmentInfo


@pytest.fixture
def synthetic_dxf_path(tmp_path: Path) -> Path:
    """Fixture que cria um arquivo DXF sintético temporário para testes."""
    dxf_file = tmp_path / "teste_eletrico.dxf"
    return CADCraftDXFAdapter.create_synthetic_dxf(dxf_file)


def test_create_and_load_synthetic_dxf(synthetic_dxf_path: Path):
    """Verifica se o arquivo DXF sintético é criado e carregado sem erros."""
    assert synthetic_dxf_path.exists()
    doc = CADCraftDXFAdapter.load_document(synthetic_dxf_path)
    assert doc is not None
    assert doc.dxfversion == "AC1024"  # R2010


def test_extract_block_inserts_with_attributes(synthetic_dxf_path: Path):
    """Verifica a extração de blocos INSERT e atributos vinculados (ATTRIB)."""
    doc = CADCraftDXFAdapter.load_document(synthetic_dxf_path)
    blocks = CADCraftDXFAdapter.extract_block_inserts(doc)

    assert len(blocks) == 5

    # Localizar Quadro de Distribuição
    qd = next((b for b in blocks if b.name == "QUADRO_DISTRIBUICAO"), None)
    assert qd is not None
    assert qd.has_attached_attribs is True
    assert qd.get_attr_value("TAG") == "QD-01"
    assert qd.get_attr_value("POTENCIA") == "15400W"
    assert qd.get_attr_value("TENSAO") == "220/127V"

    # Testar flag de visibilidade do Disjuntor Geral (flag 70 = 1 -> invisível)
    disj_attr = next((a for a in qd.attribs if a.tag == "DISJUNTOR_GERAL"), None)
    assert disj_attr is not None
    assert disj_attr.is_invisible is True
    assert disj_attr.flags & 1 == 1

    # Localizar Ponto de Iluminação
    lum = next((b for b in blocks if b.name == "LUMINARIA_TETO"), None)
    assert lum is not None
    assert lum.get_attr_value("TAG") == "LUM-SALA"
    assert lum.get_attr_value("CIRCUITO") == "1"
    assert lum.get_attr_value("COMANDO") == "a"
    assert lum.get_attr_value("POTENCIA") == "100VA"

    # Localizar Interruptor e verificar regra da NBR 5410: COMANDO presente, SEM POTENCIA
    interruptor = next((b for b in blocks if b.name == "INTERRUPTOR_SIMPLES"), None)
    assert interruptor is not None
    assert interruptor.is_switch is True
    assert interruptor.comando == "a"
    assert interruptor.circuito == "1"
    assert interruptor.potencia_raw is None
    assert interruptor.potencia_num is None
    assert len(interruptor.validate_schema()) == 0

    # Localizar Tomada TUG e verificar propriedades normalizadas
    tug = next((b for b in blocks if b.name == "TOMADA_TUG_BAIXA"), None)
    assert tug is not None
    assert tug.is_load is True
    assert tug.tipo == "TUG"
    assert tug.circuito == "2"
    assert tug.potencia_raw == "600VA"
    assert tug.potencia_num == 600.0
    assert tug.tensao == "127V"
    assert tug.fases == "1"
    assert len(tug.validate_schema()) == 0


def test_deep_inspection_fallback(synthetic_dxf_path: Path):
    """Verifica o mecanismo de fallback quando o INSERT não possui ATTRIBs vinculados."""
    doc = CADCraftDXFAdapter.load_document(synthetic_dxf_path)
    blocks = CADCraftDXFAdapter.extract_block_inserts(doc)

    cp = next((b for b in blocks if b.name == "CAIXA_PASSAGEM_FALLBACK"), None)
    assert cp is not None
    assert cp.has_attached_attribs is False

    # Deve ter recuperado da definição do bloco
    assert len(cp.attribs) >= 2
    tag_attr = next((a for a in cp.attribs if a.tag == "TAG_FALLBACK"), None)
    assert tag_attr is not None
    assert tag_attr.value == "CP-01"
    assert "ATTDEF" in tag_attr.source_type

    static_text = next((a for a in cp.attribs if "_STATIC" in a.tag), None)
    assert static_text is not None
    assert static_text.value == "CP_4x4"


def test_conduit_spatial_connectivity(synthetic_dxf_path: Path):
    """Verifica a correlação espacial entre eletrodutos e blocos usando raio de captura."""
    doc = CADCraftDXFAdapter.load_document(synthetic_dxf_path)
    blocks = CADCraftDXFAdapter.extract_block_inserts(doc)
    conduits = CADCraftDXFAdapter.extract_conduit_segments(doc)

    # Executar correlação com raio padrão de 0.20m
    CADCraftDXFAdapter.correlate_connectivity(blocks, conduits, tolerance=0.20)

    # Encontrar trecho QD -> Luminária (LINE)
    c_qd_lum = next(
        (c for c in conduits if c.entity_type == "LINE" and c.start_match.block and c.start_match.block.name == "QUADRO_DISTRIBUICAO" and c.end_match.block and c.end_match.block.name == "LUMINARIA_TETO"),
        None
    )
    assert c_qd_lum is not None
    assert c_qd_lum.is_fully_connected is True
    assert c_qd_lum.start_match.distance <= 0.20
    assert c_qd_lum.end_match.distance <= 0.20

    # Encontrar trecho Luminária -> Interruptor (LWPOLYLINE)
    c_lum_int = next(
        (c for c in conduits if c.entity_type == "LWPOLYLINE" and c.start_match.block and c.start_match.block.name == "LUMINARIA_TETO" and c.end_match.block and c.end_match.block.name == "INTERRUPTOR_SIMPLES"),
        None
    )
    assert c_lum_int is not None
    assert c_lum_int.is_fully_connected is True


def test_dangling_and_isolated_conduits(synthetic_dxf_path: Path):
    """Verifica se trechos com ponta solta e trechos isolados são identificados."""
    doc = CADCraftDXFAdapter.load_document(synthetic_dxf_path)
    blocks = CADCraftDXFAdapter.extract_block_inserts(doc)
    conduits = CADCraftDXFAdapter.extract_conduit_segments(doc)
    CADCraftDXFAdapter.correlate_connectivity(blocks, conduits, tolerance=0.20)

    # Trecho com ponta solta (Caixa de passagem para ponto vazio)
    dangling = [c for c in conduits if c.is_dangling]
    assert len(dangling) >= 1
    d_seg = dangling[0]
    assert (d_seg.start_match.block is not None and d_seg.end_match.block is None) or \
           (d_seg.start_match.block is None and d_seg.end_match.block is not None)

    # Trecho isolado (sem nenhum bloco perto)
    isolated = [c for c in conduits if c.is_isolated]
    assert len(isolated) >= 1
    iso_seg = isolated[0]
    assert iso_seg.start_match.block is None
    assert iso_seg.end_match.block is None


def test_cli_execution_diagnostic(synthetic_dxf_path: Path):
    """Executa o script CLI diretamente e verifica se retorna código 0 com saída em português."""
    import os
    env = os.environ.copy()
    env["PYTHONIOENCODING"] = "utf-8"

    result = subprocess.run(
        [sys.executable, "poc_read_cadcraft_dxf.py", str(synthetic_dxf_path), "--tolerance", "0.20"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        env=env
    )
    assert result.returncode == 0
    stdout = result.stdout

    assert "CRAFT ELE - RELATÓRIO DE DIAGNÓSTICO DXF" in stdout
    assert "QUADRO_DISTRIBUICAO" in stdout
    assert "LUMINARIA_TETO" in stdout
    assert "INTERRUPTOR_SIMPLES" in stdout
    assert "CAIXA_PASSAGEM_FALLBACK" in stdout
    assert "Inspeção Profunda / Fallback" in stdout
    assert "CONECTADO" in stdout
    assert "PONTA SOLTA" in stdout

