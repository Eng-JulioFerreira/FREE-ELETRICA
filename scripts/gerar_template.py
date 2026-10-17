"""Gera o template do CRAFT ELE (templates/CRAFT_ELE_BLOCOS.dxf) a partir do desenho de
referência dos símbolos (o Drawing1 exportado do CADCraft).

Blocos, da esquerda para a direita na parede de referência:
  INT-SIM        interruptor simples       inserção: meio da aresta que encosta na parede
  TOM-SIM-BAIXA  tomada simples baixa      inserção: meio da aresta que encosta na parede
  QDG            quadro de distribuição    inserção: meio da aresta que encosta na parede
  ILU-OCT        ponto de luz, caixa octogonal (fora da parede)   inserção: centro

Uso: python scripts/gerar_template.py <drawing1.dxf> <saida.dxf>
"""
import math
import sys

import ezdxf
from ezdxf.enums import TextEntityAlignment

origem_arq, saida = sys.argv[1], sys.argv[2]
src = ezdxf.readfile(origem_arq)
msp_src = src.modelspace()

# ------------------------------------------------------------------ geometria de origem
parede = next(e for e in msp_src.query("LINE") if e.dxf.color == 3 or e.dxf.layer == "0")
y_parede = parede.dxf.start.y
x_parede = min(parede.dxf.start.x, parede.dxf.end.x)
comp_parede = abs(parede.dxf.end.x - parede.dxf.start.x)


def nome_bloco(ins):
    return ins.dxf.name.upper()


def inserts(chave):
    return [i for i in msp_src.query("INSERT") if chave in nome_bloco(i)][0]


def geometria(ins):
    """Linhas, círculos e hachuras da inserção, em coordenadas do mundo."""
    out = []
    for v in ins.virtual_entities():
        t = v.dxftype()
        if t == "LINE":
            out.append(("line", (v.dxf.start.x, v.dxf.start.y), (v.dxf.end.x, v.dxf.end.y)))
        elif t == "CIRCLE":
            out.append(("circle", (v.dxf.center.x, v.dxf.center.y), v.dxf.radius))
        elif t == "HATCH":
            for p in v.paths:
                out.append(("hatch", [(x, y) for x, y, *_ in p.vertices]))
    return out


def contato(geo):
    """Meio da aresta horizontal que encosta na parede (a de maior comprimento em y_parede)."""
    melhor = None
    for g in geo:
        if g[0] != "line":
            continue
        (x1, y1), (x2, y2) = g[1], g[2]
        if abs(y1 - y_parede) < 1e-4 and abs(y2 - y_parede) < 1e-4:
            if melhor is None or abs(x2 - x1) > melhor[0]:
                melhor = (abs(x2 - x1), ((x1 + x2) / 2, y_parede))
    return melhor[1]


interruptor = geometria(inserts("INTERRUPTOR"))
tomada = geometria(inserts("TOMADA"))
quadro = geometria(inserts("QUADRO"))
anel = [h for h in msp_src.query("HATCH")][0]
pts = [(x, y) for p in anel.paths for x, y, *_ in p.vertices]
cx = (min(p[0] for p in pts) + max(p[0] for p in pts)) / 2
cy = (min(p[1] for p in pts) + max(p[1] for p in pts)) / 2
raios = sorted({round(max(math.hypot(x - cx, y - cy) for x, y, *_ in p.vertices), 3) for p in anel.paths})
r_int, r_ext = raios[0], raios[-1]

# ------------------------------------------------------------------ documento novo
doc = ezdxf.new("R2010", setup=True)
doc.header["$INSUNITS"] = 6  # metros
doc.header["$MEASUREMENT"] = 1
doc.header["$LTSCALE"] = 1.0
doc.linetypes.add("ELE_PISO", pattern=[0.25, 0.15, -0.10], description="Eletroduto no piso __  __  __")

camadas = {
    "ELE-PONTOS": dict(color=7, desc="CRAFT ELE: pontos elétricos (tomadas, interruptores, iluminação)"),
    "ELE-QUADROS": dict(color=2, desc="CRAFT ELE: quadros de distribuição"),
    "ELE-ELETRODUTO-PVC-MEDIA-TETO": dict(color=4, desc="Eletroduto PVC linha média embutido no teto/laje", lw=30),
    "ELE-ELETRODUTO-PVC-MEDIA-PISO": dict(color=6, desc="Eletroduto PVC linha média embutido no piso", lw=30, lt="ELE_PISO"),
    "ELE-REFERENCIA": dict(color=3, desc="Linhas de referência do template (parede)"),
}
for nome, c in camadas.items():
    layer = doc.layers.add(nome, color=c["color"], linetype=c.get("lt", "Continuous"))
    if "lw" in c:
        layer.dxf.lineweight = c["lw"]
    layer.description = c["desc"]

ALTURA_TEXTO = 0.05


def bloco(nome, geo, base, atributos):
    """`geo` em coordenadas do mundo; `base` vira a origem do bloco."""
    b = doc.blocks.new(name=nome)
    bx, by = base
    for g in geo:
        if g[0] == "line":
            b.add_line((g[1][0] - bx, g[1][1] - by), (g[2][0] - bx, g[2][1] - by))
        elif g[0] == "circle":
            b.add_circle((g[1][0] - bx, g[1][1] - by), g[2])
        elif g[0] == "hatch":
            h = b.add_hatch(color=256)
            h.paths.add_polyline_path([(x - bx, y - by) for x, y in g[1]], is_closed=True)
        elif g[0] == "anel":
            h = b.add_hatch(color=256)
            for r in (g[1], g[2]):
                # Círculo como duas meias-voltas (bulge 1).
                h.paths.add_polyline_path([(r, 0, 1), (-r, 0, 1)], is_closed=True)
    for tag, valor, prompt, modo, pos in atributos:
        # Constante também é invisível: o tipo é do bloco, não precisa aparecer no desenho.
        flags = {"visivel": 0, "invisivel": 1, "constante": 3}[modo]
        a = b.add_attdef(tag, insert=pos if pos else (0, 0), text=valor, height=ALTURA_TEXTO, dxfattribs={"prompt": prompt, "flags": flags})
        if pos and pos[0] < 0:
            a.set_placement(pos, align=TextEntityAlignment.BOTTOM_RIGHT)
    return b


base_int, base_tom, base_qd = contato(interruptor), contato(tomada), contato(quadro)
# (tag, padrão, prompt, modo, posição do texto quando visível)
bloco("INT-SIM", interruptor, base_int, [
    ("TIPO_PONTO", "INTERRUPTOR", "Tipo do ponto", "constante", None),
    ("TAG", "", "Identificação", "invisivel", None),
    ("COMANDO", "", "Letra do comando (a, b, c...)", "visivel", (0.10, 0.10)),
    ("CIRCUITO", "", "Circuito", "invisivel", None),
    ("POTENCIA", "0W", "Potência (interruptor: 0W)", "invisivel", None),
    ("TENSAO", "220V", "Tensão", "invisivel", None),
    ("NUMERO_FASES", "1", "Número de fases", "invisivel", None),
    ("ALTURA", "1,10", "Altura de instalação (m)", "invisivel", None),
])
bloco("TOM-SIM-BAIXA", tomada, base_tom, [
    ("TIPO_PONTO", "TUG", "Tipo do ponto", "constante", None),
    ("TAG", "", "Identificação", "invisivel", None),
    ("CIRCUITO", "", "Circuito", "visivel", (0.10, 0.10)),
    ("POTENCIA", "100VA", "Potência", "invisivel", None),
    ("TENSAO", "220V", "Tensão", "invisivel", None),
    ("NUMERO_FASES", "1", "Número de fases", "invisivel", None),
    ("ALTURA", "0,30", "Altura de instalação (m)", "invisivel", None),
])
bloco("QDG", quadro, base_qd, [
    ("TIPO_PONTO", "QUADRO", "Tipo do ponto", "constante", None),
    ("TAG", "QDG", "Identificação do quadro", "visivel", (0.12, -0.045)),
    ("TENSAO", "380/220V", "Tensão", "invisivel", None),
    ("NUMERO_FASES", "3", "Número de fases", "invisivel", None),
    ("ALTURA", "1,50", "Altura de instalação (m)", "invisivel", None),
])
bloco("ILU-OCT", [("anel", r_int, r_ext)], (0, 0), [
    ("TIPO_PONTO", "ILUM", "Tipo do ponto", "constante", None),
    ("TAG", "", "Identificação", "invisivel", None),
    ("POTENCIA", "100VA", "Potência", "visivel", (-0.065, r_ext + 0.025)),
    ("COMANDO", "", "Letra do comando", "visivel", (-r_ext - 0.025, -0.025)),
    ("CIRCUITO", "", "Circuito", "visivel", (r_ext + 0.025, -0.025)),
    ("TENSAO", "220V", "Tensão", "invisivel", None),
    ("NUMERO_FASES", "1", "Número de fases", "invisivel", None),
    ("ALTURA", "", "Altura de instalação (m); vazio = teto", "invisivel", None),
])

# ------------------------------------------------------------------ amostras no modelo
msp = doc.modelspace()
dx, dy = -x_parede, -y_parede  # parede de referência começando na origem
msp.add_line((0, 0), (comp_parede, 0), dxfattribs={"layer": "ELE-REFERENCIA"})
for nome, (bx, by), camada in [("INT-SIM", base_int, "ELE-PONTOS"), ("TOM-SIM-BAIXA", base_tom, "ELE-PONTOS"), ("QDG", base_qd, "ELE-QUADROS")]:
    msp.add_blockref(nome, (bx + dx, by + dy), dxfattribs={"layer": camada}).add_auto_attribs({})
msp.add_blockref("ILU-OCT", (cx + dx, cy + dy), dxfattribs={"layer": "ELE-PONTOS"}).add_auto_attribs({})
# Eletrodutos de exemplo, com legenda.
for i, (camada, texto) in enumerate([("ELE-ELETRODUTO-PVC-MEDIA-TETO", "Eletroduto PVC linha média - teto"), ("ELE-ELETRODUTO-PVC-MEDIA-PISO", "Eletroduto PVC linha média - piso")]):
    y = -0.6 - 0.35 * i
    msp.add_line((0, y), (1.5, y), dxfattribs={"layer": camada})
    msp.add_text(texto, height=0.08, dxfattribs={"layer": "ELE-REFERENCIA"}).set_placement((1.6, y - 0.04))

doc.saveas(saida)
print("ok", saida)
print("bases (relativas à parede):", [(round(b[0] + dx, 4), round(b[1] + dy, 4)) for b in (base_int, base_tom, base_qd)], "ILU", (round(cx + dx, 4), round(cy + dy, 4)), "raios", r_int, r_ext)
