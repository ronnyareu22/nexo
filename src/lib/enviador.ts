// Envía siempre el último valor y nunca deja órdenes en fila.
// Si llega un valor nuevo mientras el anterior todavía se está aplicando, se guarda solo el más reciente
// y se manda en cuanto termina el anterior. Así, al arrastrar un deslizador, la pantalla o el sonido
// siguen al dedo sin quedarse aplicando valores viejos después de soltar.
export function enviadorDelUltimo<T>(enviar: (valor: T) => Promise<unknown>, alFallar?: (error: unknown) => void) {
  let enCurso = false;
  let hayPendiente = false;
  let pendiente: T;

  return async (valor: T) => {
    pendiente = valor;
    hayPendiente = true;
    if (enCurso) return;
    enCurso = true;
    while (hayPendiente) {
      hayPendiente = false;
      try {
        await enviar(pendiente);
      } catch (e) {
        alFallar?.(e);
      }
    }
    enCurso = false;
  };
}
