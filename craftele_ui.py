#!/usr/bin/env python3
"""
================================================================================
CRAFT ELE - Interface Gráfica do Addon Elétrico para CADCraft (NBR 5410)
Arquivo: craftele_ui.py
Acionado diretamente ou pelo comando 'eletrica' dentro do CADCraft.
================================================================================
"""

from __future__ import annotations

import os
import sys
import time
from pathlib import Path
import tkinter as tk
from tkinter import ttk, filedialog, messagebox

# Permitir importação de src
sys.path.insert(0, str(Path(__file__).parent))

from src.adapters.dxf_adapter import (
    CADCraftDXFAdapter,
    BlockInsertInfo,
    ConduitSegmentInfo,
    DXFDiagnosticReport,
)


class CraftEleUI(tk.Tk):
    """Janela Gráfica Companion do Addon CRAFT ELE para o CADCraft."""

    TRIGGER_FILE = Path(os.getenv("APPDATA", "")) / "CADCraft" / "craftele_trigger.flag"

    def __init__(self, initial_dxf: str | Path | None = None):
        super().__init__()

        self.title("⚡ CRAFT ELE - Addon de Instalações Elétricas (NBR 5410) | CADCraft")
        self.geometry("1100x720")
        self.minsize(900, 600)

        # Configurar Estilos Dark Moderno
        self.configure(bg="#1e1e1e")
        self._setup_styles()

        # Estado do Sistema
        self.current_dxf_path: Optional[Path] = Path(initial_dxf) if initial_dxf else None
        self.report: Optional[DXFDiagnosticReport] = None
        self.last_trigger_mtime: float = 0.0

        # Construir Interface
        self._build_header()
        self._build_toolbar()
        self._build_main_content()
        self._build_statusbar()

        # Iniciar monitoramento do comando 'eletrica' do CADCraft
        self._init_trigger_watcher()

        # Se já tiver um arquivo inicial ou o sintético existir, carregar
        if self.current_dxf_path and self.current_dxf_path.is_file():
            self.load_dxf(self.current_dxf_path)
        else:
            default_sample = Path("data") / "circuito_teste_sintetico.dxf"
            if default_sample.is_file():
                self.load_dxf(default_sample)

    def _setup_styles(self) -> None:
        style = ttk.Style(self)
        style.theme_use("clam")

        # Cores Dark
        bg_dark = "#1e1e1e"
        bg_card = "#252526"
        fg_text = "#d4d4d4"
        accent = "#007acc"
        accent_hover = "#1f8ad2"

        style.configure(".", background=bg_dark, foreground=fg_text)
        style.configure("TFrame", background=bg_dark)
        style.configure("Card.TFrame", background=bg_card, relief="flat")

        # Botões
        style.configure(
            "Primary.TButton",
            background=accent,
            foreground="#ffffff",
            font=("Segoe UI", 9, "bold"),
            padding=(12, 6),
            borderwidth=0,
        )
        style.map("Primary.TButton", background=[("active", accent_hover)])

        style.configure(
            "Secondary.TButton",
            background="#333333",
            foreground="#ffffff",
            font=("Segoe UI", 9),
            padding=(10, 5),
            borderwidth=0,
        )
        style.map("Secondary.TButton", background=[("active", "#444444")])

        # Notebook (Abas)
        style.configure(
            "TNotebook",
            background=bg_dark,
            borderwidth=0,
            tabmargins=[2, 5, 2, 0],
        )
        style.configure(
            "TNotebook.Tab",
            background="#2d2d2d",
            foreground="#cccccc",
            padding=[15, 6],
            font=("Segoe UI", 9, "bold"),
            borderwidth=0,
        )
        style.map(
            "TNotebook.Tab",
            background=[("selected", bg_card)],
            foreground=[("selected", "#ffffff")],
        )

        # Treeview (Tabelas)
        style.configure(
            "Treeview",
            background="#252526",
            foreground="#d4d4d4",
            fieldbackground="#252526",
            rowheight=26,
            font=("Segoe UI", 9),
            borderwidth=0,
        )
        style.configure(
            "Treeview.Heading",
            background="#333333",
            foreground="#ffffff",
            font=("Segoe UI", 9, "bold"),
            padding=(6, 4),
            borderwidth=0,
        )
        style.map(
            "Treeview",
            background=[("selected", "#094771")],
            foreground=[("selected", "#ffffff")],
        )

    def _build_header(self) -> None:
        header_frame = tk.Frame(self, bg="#181818", height=60)
        header_frame.pack(fill="x", side="top")

        # Logo / Título
        title_box = tk.Frame(header_frame, bg="#181818")
        title_box.pack(side="left", padx=16, pady=10)

        lbl_logo = tk.Label(
            title_box,
            text="⚡ CRAFT ELE",
            font=("Segoe UI", 14, "bold"),
            fg="#4ec9b0",
            bg="#181818",
        )
        lbl_logo.pack(side="left")

        lbl_sub = tk.Label(
            title_box,
            text=" |  Motor NBR 5410 & Addon do CADCraft",
            font=("Segoe UI", 10),
            fg="#9cdcfe",
            bg="#181818",
        )
        lbl_sub.pack(side="left")

        # Status Badge do CADCraft
        badge_box = tk.Frame(header_frame, bg="#181818")
        badge_box.pack(side="right", padx=16, pady=10)

        self.lbl_cadcraft_status = tk.Label(
            badge_box,
            text="🟢 CADCraft Integrado (Comando 'eletrica' ativo)",
            font=("Segoe UI", 9, "bold"),
            fg="#4ec9b0",
            bg="#223322",
            padx=10,
            pady=4,
            relief="flat",
        )
        self.lbl_cadcraft_status.pack()

    def _build_toolbar(self) -> None:
        bar = tk.Frame(self, bg="#252526", height=45)
        bar.pack(fill="x", padx=12, pady=(10, 6))

        # Botão Abrir DXF
        btn_open = ttk.Button(
            bar,
            text="📂 Abrir Arquivo DXF...",
            style="Primary.TButton",
            command=self._on_click_open_dxf,
        )
        btn_open.pack(side="left", padx=(8, 4), pady=6)

        # Botão Recarregar
        btn_reload = ttk.Button(
            bar,
            text="🔄 Recarregar Desenho",
            style="Secondary.TButton",
            command=self._on_click_reload,
        )
        btn_reload.pack(side="left", padx=4, pady=6)

        # Separador visual
        sep = tk.Frame(bar, width=1, bg="#444444", height=24)
        sep.pack(side="left", padx=10, pady=8)

        # Tolerância Espacial
        lbl_tol = tk.Label(
            bar,
            text="Raio de Captura:",
            font=("Segoe UI", 9),
            bg="#252526",
            fg="#d4d4d4",
        )
        lbl_tol.pack(side="left", padx=(4, 2))

        self.entry_tol = tk.Entry(
            bar,
            width=6,
            font=("Segoe UI", 9),
            bg="#1e1e1e",
            fg="#ffffff",
            insertbackground="#ffffff",
            relief="flat",
        )
        self.entry_tol.insert(0, "0.20")
        self.entry_tol.pack(side="left", padx=2)

        lbl_unit = tk.Label(
            bar,
            text="m",
            font=("Segoe UI", 9),
            bg="#252526",
            fg="#888888",
        )
        lbl_unit.pack(side="left", padx=(0, 10))

        # Arquivo Atual Label
        self.lbl_file_info = tk.Label(
            bar,
            text="Nenhum arquivo carregado",
            font=("Segoe UI", 9, "italic"),
            bg="#252526",
            fg="#9cdcfe",
        )
        self.lbl_file_info.pack(side="right", padx=12)

    def _build_main_content(self) -> None:
        self.notebook = ttk.Notebook(self)
        self.notebook.pack(fill="both", expand=True, padx=12, pady=6)

        # Aba 1: Blocos e Parâmetros Elétricos
        self.tab_blocks = ttk.Frame(self.notebook, style="Card.TFrame")
        self.notebook.add(self.tab_blocks, text=" 🔌 Blocos & Parâmetros Elétricos ")
        self._build_blocks_tab(self.tab_blocks)

        # Aba 2: Eletrodutos e Conectividade
        self.tab_conduits = ttk.Frame(self.notebook, style="Card.TFrame")
        self.notebook.add(self.tab_conduits, text=" 📐 Eletrodutos & Topologia ")
        self._build_conduits_tab(self.tab_conduits)

        # Aba 3: Resumo da Instalação e Quadro de Cargas
        self.tab_calc = ttk.Frame(self.notebook, style="Card.TFrame")
        self.notebook.add(self.tab_calc, text=" 📊 Quadro de Cargas & NBR 5410 ")
        self._build_calc_tab(self.tab_calc)

    def _build_blocks_tab(self, parent: ttk.Frame) -> None:
        # Split horizontal: Tabela em cima, Detalhes embaixo
        paned = tk.PanedWindow(
            parent,
            orient="vertical",
            bg="#1e1e1e",
            sashrelief="flat",
            sashwidth=4,
        )
        paned.pack(fill="both", expand=True, padx=6, pady=6)

        # Frame da Tabela
        tbl_frame = tk.Frame(paned, bg="#252526")
        paned.add(tbl_frame, minsize=250)

        # Treeview de Blocos
        cols = (
            "handle",
            "nome",
            "tipo",
            "potencia",
            "tensao",
            "fases",
            "circuito",
            "comando",
            "status",
        )
        self.tree_blocks = ttk.Treeview(tbl_frame, columns=cols, show="headings")

        self.tree_blocks.heading("handle", text="Handle")
        self.tree_blocks.heading("nome", text="Bloco")
        self.tree_blocks.heading("tipo", text="Tipo")
        self.tree_blocks.heading("potencia", text="Potência")
        self.tree_blocks.heading("tensao", text="Tensão")
        self.tree_blocks.heading("fases", text="Fases")
        self.tree_blocks.heading("circuito", text="Circuito")
        self.tree_blocks.heading("comando", text="Comando")
        self.tree_blocks.heading("status", text="Status NBR 5410")

        self.tree_blocks.column("handle", width=80, anchor="center")
        self.tree_blocks.column("nome", width=170, anchor="w")
        self.tree_blocks.column("tipo", width=120, anchor="center")
        self.tree_blocks.column("potencia", width=100, anchor="e")
        self.tree_blocks.column("tensao", width=90, anchor="center")
        self.tree_blocks.column("fases", width=60, anchor="center")
        self.tree_blocks.column("circuito", width=80, anchor="center")
        self.tree_blocks.column("comando", width=80, anchor="center")
        self.tree_blocks.column("status", width=180, anchor="w")

        scrollbar = ttk.Scrollbar(tbl_frame, orient="vertical", command=self.tree_blocks.yview)
        self.tree_blocks.configure(yscrollcommand=scrollbar.set)

        self.tree_blocks.pack(side="left", fill="both", expand=True)
        scrollbar.pack(side="right", fill="y")

        self.tree_blocks.bind("<<TreeviewSelect>>", self._on_block_selected)

        # Frame de Detalhes do Bloco Selecionado
        det_frame = tk.Frame(paned, bg="#252526")
        paned.add(det_frame, minsize=140)

        lbl_det_title = tk.Label(
            det_frame,
            text="Atributos Brutos do Bloco e Diagnóstico:",
            font=("Segoe UI", 9, "bold"),
            bg="#252526",
            fg="#9cdcfe",
        )
        lbl_det_title.pack(anchor="w", padx=10, pady=(6, 2))

        self.txt_block_details = tk.Text(
            det_frame,
            height=6,
            bg="#1e1e1e",
            fg="#d4d4d4",
            font=("Consolas", 9),
            relief="flat",
            padx=8,
            pady=6,
        )
        self.txt_block_details.pack(fill="both", expand=True, padx=10, pady=(0, 6))

    def _build_conduits_tab(self, parent: ttk.Frame) -> None:
        tbl_frame = tk.Frame(parent, bg="#252526")
        tbl_frame.pack(fill="both", expand=True, padx=6, pady=6)

        cols = ("handle", "tipo", "layer", "comp", "inicio", "fim", "status")
        self.tree_conduits = ttk.Treeview(tbl_frame, columns=cols, show="headings")

        self.tree_conduits.heading("handle", text="Handle")
        self.tree_conduits.heading("tipo", text="Tipo")
        self.tree_conduits.heading("layer", text="Layer")
        self.tree_conduits.heading("comp", text="Comp. (m)")
        self.tree_conduits.heading("inicio", text="Início (Bloco Conectado)")
        self.tree_conduits.heading("fim", text="Fim (Bloco Conectado)")
        self.tree_conduits.heading("status", text="Status")

        self.tree_conduits.column("handle", width=80, anchor="center")
        self.tree_conduits.column("tipo", width=100, anchor="center")
        self.tree_conduits.column("layer", width=120, anchor="w")
        self.tree_conduits.column("comp", width=90, anchor="e")
        self.tree_conduits.column("inicio", width=240, anchor="w")
        self.tree_conduits.column("fim", width=240, anchor="w")
        self.tree_conduits.column("status", width=130, anchor="center")

        scrollbar = ttk.Scrollbar(tbl_frame, orient="vertical", command=self.tree_conduits.yview)
        self.tree_conduits.configure(yscrollcommand=scrollbar.set)

        self.tree_conduits.pack(side="left", fill="both", expand=True)
        scrollbar.pack(side="right", fill="y")

    def _build_calc_tab(self, parent: ttk.Frame) -> None:
        frame = tk.Frame(parent, bg="#252526")
        frame.pack(fill="both", expand=True, padx=16, pady=16)

        lbl = tk.Label(
            frame,
            text="Resumo da Instalação Elétrica (NBR 5410)",
            font=("Segoe UI", 12, "bold"),
            bg="#252526",
            fg="#4ec9b0",
        )
        lbl.pack(anchor="w", pady=(0, 10))

        self.lbl_calc_summary = tk.Label(
            frame,
            text="Carregue uma planta do CADCraft para visualizar o dimensionamento dos circuitos.",
            font=("Segoe UI", 10),
            bg="#252526",
            fg="#d4d4d4",
            justify="left",
        )
        self.lbl_calc_summary.pack(anchor="w")

    def _build_statusbar(self) -> None:
        self.statusbar = tk.Label(
            self,
            text="Pronto. Digite 'eletrica' no CADCraft ou abra um arquivo DXF.",
            font=("Segoe UI", 9),
            bg="#007acc",
            fg="#ffffff",
            anchor="w",
            padx=10,
            pady=3,
        )
        self.statusbar.pack(fill="x", side="bottom")

    # -----------------------------------------------------------------
    # Monitoramento do Comando 'eletrica' do CADCraft
    # -----------------------------------------------------------------
    def _init_trigger_watcher(self) -> None:
        """Verifica a cada 500ms se o comando 'eletrica' foi acionado no CADCraft."""
        self._check_trigger_file()
        self.after(500, self._init_trigger_watcher)

    def _check_trigger_file(self) -> None:
        if not self.TRIGGER_FILE.exists():
            return

        try:
            mtime = self.TRIGGER_FILE.stat().st_mtime
            if mtime > self.last_trigger_mtime:
                self.last_trigger_mtime = mtime
                # Ler conteúdo (caminho do arquivo ativo passado pelo CADCraft)
                try:
                    content = self.TRIGGER_FILE.read_text(encoding="utf-8").strip()
                except Exception:
                    content = ""

                # Trazer janela para frente
                self.lift()
                self.attributes("-topmost", True)
                self.after(100, lambda: self.attributes("-topmost", False))
                self.focus_force()

                self.statusbar.config(
                    text=f"⚡ Comando 'eletrica' recebido do CADCraft às {time.strftime('%H:%M:%S')}!",
                    bg="#4ec9b0",
                    fg="#1e1e1e",
                )

                if content and Path(content).is_file():
                    self.load_dxf(Path(content))
        except Exception as e:
            pass

    # -----------------------------------------------------------------
    # Ações e Carregamento de Arquivo
    # -----------------------------------------------------------------
    def _on_click_open_dxf(self) -> None:
        f = filedialog.askopenfilename(
            title="Selecionar Planta Elétrica do CADCraft (DXF)",
            filetypes=[("Arquivos DXF", "*.dxf"), ("Todos os Arquivos", "*.*")],
        )
        if f:
            self.load_dxf(Path(f))

    def _on_click_reload(self) -> None:
        if self.current_dxf_path and self.current_dxf_path.is_file():
            self.load_dxf(self.current_dxf_path)
        else:
            self.statusbar.config(text="Nenhum arquivo ativo para recarregar.")

    def load_dxf(self, path: Path) -> None:
        """Carrega e executa o diagnóstico no arquivo DXF."""
        try:
            tolerance = float(self.entry_tol.get())
        except ValueError:
            tolerance = 0.20

        try:
            doc = CADCraftDXFAdapter.load_document(path)
            self.report = CADCraftDXFAdapter.run_diagnostics(
                doc=doc,
                filename=path.name,
                tolerance=tolerance,
            )
            self.current_dxf_path = path

            # Atualizar cabeçalho e status
            self.lbl_file_info.config(text=f"Planta: {path.name} ({self.report.dxf_version})")
            self.statusbar.config(
                text=f"Arquivo '{path.name}' carregado. {len(self.report.inserts)} blocos e {len(self.report.conduits)} eletrodutos analisados.",
                bg="#007acc",
                fg="#ffffff",
            )

            # Preencher Tabelas
            self._populate_blocks(self.report.inserts)
            self._populate_conduits(self.report.conduits)
            self._populate_calc_summary(self.report)

        except Exception as e:
            messagebox.showerror("Erro ao carregar DXF", f"Falha ao ler o arquivo:\n{e}")

    def _populate_blocks(self, blocks: List[BlockInsertInfo]) -> None:
        self.tree_blocks.delete(*self.tree_blocks.get_children())
        self.txt_block_details.delete("1.0", "end")

        for b in blocks:
            warnings = b.validate_schema()
            if not warnings:
                status = "✅ Válido (NBR 5410)"
            else:
                status = f"⚠️ {warnings[0]}"

            pot_display = b.potencia_raw or "-"
            fases_display = b.fases or "-"
            tensao_display = b.tensao or "-"
            cmd_display = b.comando or "-"
            circ_display = b.circuito or "-"

            self.tree_blocks.insert(
                "",
                "end",
                iid=b.handle,
                values=(
                    f"#{b.handle}",
                    b.name,
                    b.tipo,
                    pot_display,
                    tensao_display,
                    fases_display,
                    circ_display,
                    cmd_display,
                    status,
                ),
            )

    def _populate_conduits(self, conduits: List[ConduitSegmentInfo]) -> None:
        self.tree_conduits.delete(*self.tree_conduits.get_children())

        for c in conduits:
            if c.is_fully_connected:
                st = "✅ Conectado"
            elif c.is_dangling:
                st = "⚠️ Ponta Solta"
            else:
                st = "❌ Desconectado"

            start_txt = (
                f"{c.start_match.block.name} (#{c.start_match.block.handle})"
                if c.start_match and c.start_match.block
                else f"Livre ({c.start_point[0]:.2f}, {c.start_point[1]:.2f})"
            )

            end_txt = (
                f"{c.end_match.block.name} (#{c.end_match.block.handle})"
                if c.end_match and c.end_match.block
                else f"Livre ({c.end_point[0]:.2f}, {c.end_point[1]:.2f})"
            )

            self.tree_conduits.insert(
                "",
                "end",
                iid=c.handle,
                values=(
                    f"#{c.handle}",
                    c.entity_type,
                    c.layer,
                    f"{c.length_2d:.2f}",
                    start_txt,
                    end_txt,
                    st,
                ),
            )

    def _populate_calc_summary(self, report: DXFDiagnosticReport) -> None:
        total_pot_w = 0.0
        circuitos: Dict[str, float] = {}

        for b in report.inserts:
            if b.is_load and b.potencia_num:
                total_pot_w += b.potencia_num
                c_id = b.circuito or "Indefinido"
                circuitos[c_id] = circuitos.get(c_id, 0.0) + b.potencia_num

        total_conduits = len(report.conduits)
        connected = sum(1 for c in report.conduits if c.is_fully_connected)

        summary_text = (
            f"• Total de Blocos Elétricos : {len(report.inserts)}\n"
            f"• Carga Total Mapeada       : {total_pot_w:.1f} W (VA)\n"
            f"• Circuitos Detectados      : {len(circuitos)} ({', '.join(circuitos.keys())})\n"
            f"• Trechos de Eletrodutos    : {total_conduits} ({connected} totalmente conectados)\n\n"
            f"Cargas por Circuito:\n"
        )
        for c_id, pot in circuitos.items():
            summary_text += f"   - Circuito {c_id}: {pot:.1f} W/VA\n"

        self.lbl_calc_summary.config(text=summary_text)

    def _on_block_selected(self, event) -> None:
        selected = self.tree_blocks.selection()
        if not selected or not self.report:
            return

        handle = selected[0]
        block = next((b for b in self.report.inserts if b.handle == handle), None)
        if not block:
            return

        self.txt_block_details.delete("1.0", "end")
        details = (
            f"Bloco: {block.name} (Handle: #{block.handle})\n"
            f"Camada: {block.layer} | Posição: ({block.x:.3f}, {block.y:.3f}, {block.z:.3f})\n"
            f"Origem dos Atributos: {'ATTRIB Nativo no INSERT' if block.has_attached_attribs else 'Definição de Bloco (Fallback)'}\n"
            f"Atributos extraídos:\n"
        )
        for att in block.attribs:
            details += f"  • {att.tag} = '{att.value}' (Flags 70: {att.flags}, Origem: {att.source_type})\n"

        warnings = block.validate_schema()
        if warnings:
            details += "\nAlertas NBR 5410:\n"
            for w in warnings:
                details += f"  [!] {w}\n"
        else:
            details += "\n[OK] Atributos consistentes com a norma NBR 5410.\n"

        self.txt_block_details.insert("1.0", details)


def main() -> None:
    initial_file = sys.argv[1] if len(sys.argv) > 1 else None
    app = CraftEleUI(initial_dxf=initial_file)
    app.mainloop()


if __name__ == "__main__":
    main()
