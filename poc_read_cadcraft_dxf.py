#!/usr/bin/env python3
"""
================================================================================
CRAFT ELE - Motor Open Source de Projetos Elétricos (NBR 5410)
Script de Diagnóstico e Prova de Conceito: Leitura e Conectividade DXF (CADCraft)
Arquivo: poc_read_cadcraft_dxf.py
================================================================================
"""

from __future__ import annotations

import argparse
import io
import os
import sys
from pathlib import Path

# Configurar stdout/stderr para UTF-8 no Windows
if sys.platform == "win32":
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
        sys.stderr.reconfigure(encoding="utf-8", errors="replace")
    except AttributeError:
        sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
        sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8", errors="replace")

# Permitir importação de src mesmo se executado diretamente da raiz
sys.path.insert(0, str(Path(__file__).parent))


from src.adapters.dxf_adapter import CADCraftDXFAdapter, DXFDiagnosticReport, BlockInsertInfo, ConduitSegmentInfo


def format_header(title: str, width: int = 85) -> str:
    line = "=" * width
    return f"\n{line}\n  {title.center(width - 4)}\n{line}"


def format_section(title: str, width: int = 85) -> str:
    line = "-" * width
    return f"\n{line}\n  [+] {title}\n{line}"


def print_report(report: DXFDiagnosticReport) -> None:
    """Imprime o relatório formatado em português no terminal."""
    print(format_header("CRAFT ELE - RELATÓRIO DE DIAGNÓSTICO DXF (CADCraft / NBR 5410)"))

    print(f" Arquivo analisado : {report.filename}")
    print(f" Versão do DXF     : {report.dxf_version}")
    print(f" Definições de bloco: {report.total_blocks_defined}")
    print(f" Blocos inseridos  : {report.total_inserts}")
    print(f" Trechos LINE      : {report.total_lines}")
    print(f" Trechos LWPOLYLINE: {report.total_lwpolylines}")
    print(f" Tolerância Raio   : {report.tolerance_used:.2f} m (unidades)")
    print(f" Modo de Atributos : {report.attribute_mode_detected}")

    # -------------------------------------------------------------
    # 1. BLOCOS INSERIDOS E ATRIBUTOS
    # -------------------------------------------------------------
    print(format_section("1. INVENTÁRIO DE ELEMENTOS ELÉTRICOS (INSERT & ATTRIB)"))

    if not report.inserts:
        print("  (Nenhum bloco INSERT encontrado no Modelspace)")
    else:
        for idx, block in enumerate(report.inserts, start=1):
            status_attrib = "ATTRIB NATIVO" if block.has_attached_attribs else "SEM ATTRIB VINCULADO"
            print(f"\n  [{idx:02d}] Bloco: '{block.name}' | Handle: #{block.handle} | Layer: '{block.layer}'")
            print(f"       Coordenadas (X, Y, Z): ({block.x:.3f}, {block.y:.3f}, {block.z:.3f}) | Rot: {block.rotation:.1f}°")
            print(f"       Status de Atributos   : {status_attrib}")

            if block.attribs:
                print("       Atributos extraídos:")
                for att in block.attribs:
                    vis_label = "INVISÍVEL" if att.is_invisible else "VISÍVEL"
                    const_label = "CONSTANTE" if att.is_constant else "VARIÁVEL"
                    print(
                        f"         * {att.tag:<16} = '{att.value}' "
                        f"[Origem: {att.source_type} | Flags 70: {att.flags} ({vis_label}, {const_label})]"
                    )
            else:
                print("       (!) Nenhum atributo ou texto associado encontrado.")

            if not block.has_attached_attribs and block.definition_fallback_entities:
                print("       [Inspeção Profunda / Fallback na Definição do Bloco]:")
                for item in block.definition_fallback_entities:
                    print(f"         > Entidade encontrada na definição: {item}")

    # -------------------------------------------------------------
    # 2. TUBULAÇÃO E CONECTIVIDADE TOPOLÓGICA
    # -------------------------------------------------------------
    print(format_section(f"2. ELETRODUTOS E CONECTIVIDADE ESPACIAL (Raio: {report.tolerance_used:.2f}m)"))

    if not report.conduits:
        print("  (Nenhum segmento de eletroduto LINE ou LWPOLYLINE encontrado)")
    else:
        print(f"  {'Handle':<8} {'Tipo':<12} {'Layer':<14} {'Comp(m)':<8} {'Início -> Conexão':<26} {'Fim -> Conexão':<26} {'Status'}")
        print("  " + "-" * 105)

        for cond in report.conduits:
            # Formatando início
            if cond.start_match and cond.start_match.block:
                b = cond.start_match.block
                start_desc = f"{b.name} (#{b.handle}, Δ={cond.start_match.distance:.3f}m)"
            else:
                p = cond.start_point
                start_desc = f"LIVRE ({p[0]:.2f}, {p[1]:.2f})"

            # Formatando fim
            if cond.end_match and cond.end_match.block:
                b = cond.end_match.block
                end_desc = f"{b.name} (#{b.handle}, Δ={cond.end_match.distance:.3f}m)"
            else:
                p = cond.end_point
                end_desc = f"LIVRE ({p[0]:.2f}, {p[1]:.2f})"

            # Status
            if cond.is_fully_connected:
                status_str = "[V] CONECTADO"
            elif cond.is_dangling:
                status_str = "[!] PONTA SOLTA"
            else:
                status_str = "[X] DESCONECTADO"

            print(
                f"  #{cond.handle:<7} {cond.entity_type:<12} {cond.layer:<14} "
                f"{cond.length_2d:<8.2f} {start_desc:<26} {end_desc:<26} {status_str}"
            )

    # -------------------------------------------------------------
    # 3. RESUMO ESTATÍSTICO E DIAGNÓSTICO TÉCNICO
    # -------------------------------------------------------------
    print(format_section("3. RESUMO ESTATÍSTICO & DIAGNÓSTICO DO MOTOR ELÉTRICO"))

    total_conduits = len(report.conduits)
    connected = sum(1 for c in report.conduits if c.is_fully_connected)
    dangling = sum(1 for c in report.conduits if c.is_dangling)
    isolated = sum(1 for c in report.conduits if c.is_isolated)

    pct_connected = (connected / total_conduits * 100) if total_conduits > 0 else 0.0

    print(f"  - Total de eletrodutos mapeados  : {total_conduits}")
    print(f"  - Trechos totalmente conectados  : {connected} ({pct_connected:.1f}%)")
    print(f"  - Trechos com ponta solta        : {dangling}")
    print(f"  - Trechos isolados / sem blocos  : {isolated}")

    print("\n  [DIAGNÓSTICO TÉCNICO PARA O CADCraft / NBR 5410]:")
    if report.attribute_mode_detected == "NATIVE_ATTRIB":
        print("  -> SUCESSO: O arquivo utiliza atributos nativos ATTRIB vinculados a cada INSERT.")
        print("     Compatibilidade máxima com a extração de potência, circuito e comando!")
    elif report.attribute_mode_detected == "BLOCK_DEF_FALLBACK":
        print("  -> ALERTA (Fallback Ativo): Os blocos usam definições ATTDEF/TEXT globais na definição.")
        print("     O motor elétrico conseguiu recuperar os parâmetros pelo mecanismo de fallback.")
    elif report.attribute_mode_detected == "MIXED":
        print("  -> MISTO: Foram encontrados tanto blocos com ATTRIB nativo quanto blocos dependentes de Fallback.")
        print("     O adaptador DXF suporta ambos simultaneamente.")
    else:
        print("  -> AVISO: Não foram detectados atributos nos blocos.")

    if dangling > 0:
        print(f"  -> ATENÇÃO TOPOLÓGICA: Existem {dangling} eletroduto(s) com extremidade solta.")
        print("     Verifique se o raio de captura (tolerância) precisa ser ajustado ou se o desenho possui caixas não conectadas.")
    else:
        print("  -> TOPOLOGIA CONSISTENTE: Todos os eletrodutos mapeados formam pares entre blocos.")

    print(format_header("FIM DO RELATÓRIO DE DIAGNÓSTICO"))


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Diagnóstico e leitura de DXF (CADCraft / NBR 5410) - CRAFT ELE"
    )
    parser.add_argument(
        "dxf_file",
        nargs="?",
        default=None,
        help="Caminho do arquivo DXF. Se omitido, gera e analisa um arquivo de teste sintético.",
    )
    parser.add_argument(
        "-t",
        "--tolerance",
        type=float,
        default=CADCraftDXFAdapter.DEFAULT_CAPTURE_RADIUS,
        help="Tolerância de captura espacial (raio em metros ou unidades de desenho). Padrão: 0.20",
    )
    parser.add_argument(
        "--generate-only",
        action="store_true",
        help="Apenas gera o arquivo DXF sintético de demonstração e encerra.",
    )

    args = parser.parse_args()

    default_synthetic_path = Path("data") / "circuito_teste_sintetico.dxf"

    # Caso nenhum arquivo seja passado, gerar ou usar o sintético
    if not args.dxf_file:
        print(f"[!] Nenhum arquivo DXF fornecido via CLI.")
        print(f"[*] Gerando arquivo DXF de teste sintético em: '{default_synthetic_path.resolve()}'...")
        dxf_path = CADCraftDXFAdapter.create_synthetic_dxf(default_synthetic_path)
        print(f"[OK] Arquivo sintético criado com sucesso!")
        if args.generate_only:
            return 0
    else:
        dxf_path = Path(args.dxf_file)
        if not dxf_path.is_file():
            print(f"[ERRO] Arquivo não encontrado: {dxf_path.resolve()}", file=sys.stderr)
            return 1

    # Carregar e diagnosticar
    try:
        doc = CADCraftDXFAdapter.load_document(dxf_path)
        report = CADCraftDXFAdapter.run_diagnostics(
            doc=doc,
            filename=str(dxf_path.name),
            tolerance=args.tolerance,
        )
        print_report(report)
        return 0
    except Exception as e:
        print(f"[ERRO CRÍTICO] Falha ao processar o arquivo DXF: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        return 1


if __name__ == "__main__":
    sys.exit(main())
