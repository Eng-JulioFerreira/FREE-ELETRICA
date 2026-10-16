;;; ==============================================================================
;;; CRAFT ELE - Addon de Instalacoes Eletricas (NBR 5410) para CADCraft
;;; Arquivo: craftele.lsp / acaddoc.lsp
;;; Define o comando 'eletrica' diretamente na linha de comando do CADCraft.
;;; ==============================================================================

(defun c:eletrica ()
  (setq flag_path "C:/Users/Public/craftele_trigger.flag")
  (setq f (open flag_path "w"))
  (if f
    (progn
      (write-line (getvar "DWGNAME") f)
      (close f)
      (princ "\n[CRAFT ELE] Sinal enviado com sucesso! A janela do CRAFT ELE esta aberta.")
    )
    (progn
      ;; Tentativa alternativa via caminho de projeto
      (setq f2 (open "C:/Users/JLIOFE~1/Desktop/CRAFTE~1/craftele_trigger.flag" "w"))
      (if f2
        (progn
          (write-line (getvar "DWGNAME") f2)
          (close f2)
          (princ "\n[CRAFT ELE] Sinal enviado com sucesso! A janela do CRAFT ELE esta aberta.")
        )
        (princ "\n[CRAFT ELE] Erro: Nao foi possivel escrever no arquivo de comunicacao.")
      )
    )
  )
  (princ)
)

(defun c:craftele ()
  (c:eletrica)
)

(princ "\n[CRAFT ELE] Modulo eletrico NBR 5410 carregado. Digite 'eletrica' para abrir a janela.")
(princ)
