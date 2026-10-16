;;; ==============================================================================
;;; CRAFT ELE - Addon de Instalações Elétricas (NBR 5410) para CADCraft
;;; Arquivo: craftele.lsp / acaddoc.lsp
;;; Define o comando 'eletrica' diretamente na linha de comando do CADCraft.
;;; ==============================================================================

(defun c:eletrica ()
  (setq flag_path (strcat (getenv "APPDATA") "\\CADCraft\\craftele_trigger.flag"))
  (setq f (open flag_path "w"))
  (if f
    (progn
      ;; Envia o caminho do desenho atual aberto no CADCraft
      (write-line (strcat (getvar "dwgprefix") (getvar "dwgname")) f)
      (close f)
      (princ "\n[CRAFT ELE] Sinal de abertura enviado! A janela grafica esta sendo exibida.")
    )
    (princ "\n[CRAFT ELE] Aviso: Nao foi possivel registrar o arquivo de trigger.")
  )
  (princ)
)

(defun c:craftele ()
  (c:eletrica)
)

(princ "\n[CRAFT ELE] Modulo eletrico NBR 5410 carregado. Digite 'eletrica' para abrir a janela.")
(princ)
