#!/usr/bin/env python3
"""Ventana destino para la prueba de extremo a extremo.

Abre una ventana con un cuadro de texto enfocado, espera a que Dictado pegue algo, guarda el
contenido en el archivo indicado y se cierra sola.

Uso: python3 scripts/paste_target.py <archivo_salida> [segundos_max]
"""
import sys
import time
import tkinter as tk

out = sys.argv[1]
timeout = float(sys.argv[2]) if len(sys.argv) > 2 else 60.0

root = tk.Tk()
root.title("Dictado — destino de pegado (prueba automática)")
root.geometry("680x220+240+240")
label = tk.Label(root, text="Esta ventana la abrió la prueba automática de Dictado; se cerrará sola.")
label.pack(anchor="w", padx=10, pady=(8, 0))
text = tk.Text(root, font=("Helvetica", 15), wrap="word")
text.pack(fill="both", expand=True, padx=10, pady=10)
text.focus_set()
root.lift()
root.attributes("-topmost", True)
root.after(800, lambda: root.attributes("-topmost", False))
root.focus_force()

start = time.time()
state = {"len": 0, "changed": None}


def poll():
    content = text.get("1.0", "end-1c")
    if len(content) != state["len"]:
        state["len"] = len(content)
        state["changed"] = time.time()
    now = time.time()
    if (state["changed"] and now - state["changed"] > 3) or now - start > timeout:
        with open(out, "w", encoding="utf-8") as f:
            f.write(content)
        root.destroy()
        return
    root.after(200, poll)


root.after(200, poll)
root.mainloop()
