<!-- Deslizador 0–100: pista hundida, relleno con el color de su familia y perilla blanca. Funciona con teclado. -->
<script lang="ts">
  let {
    valor,
    etiqueta,
    color = 'celeste',
    apagado = false,
    tono,
    oncambio,
    ontocando,
  }: {
    valor: number;
    etiqueta: string;
    /** celeste para sonido, amarillo para brillo, verde para batería y conexión, primario para lo demás */
    color?: 'celeste' | 'amarillo' | 'verde' | 'primario';
    apagado?: boolean;
    oncambio: (v: number) => void;
    ontocando?: (tocando: boolean) => void;
    /** Color del relleno a mano (por ejemplo, el tono del brillo); si no, el de la familia. */
    tono?: string;
  } = $props();
</script>

<input
  type="range"
  min="0"
  max="100"
  step="1"
  value={valor}
  aria-label={etiqueta}
  class:apagado
  style:--parte={valor / 100}
  style:--relleno={tono ?? `var(--${color})`}
  oninput={(e) => oncambio(Number(e.currentTarget.value))}
  onpointerdown={(e) => {
    // Captura el puntero: aunque sueltes fuera de la barra, el "soltar" llega igual.
    e.currentTarget.setPointerCapture(e.pointerId);
    ontocando?.(true);
  }}
  onpointerup={() => ontocando?.(false)}
  onpointercancel={() => ontocando?.(false)}
  onchange={() => ontocando?.(false)}
/>

<style>
  input {
    --lleno: calc(10px + (100% - 20px) * var(--parte));
    -webkit-appearance: none;
    appearance: none;
    display: block;
    width: 100%;
    height: 24px;
    margin: 0;
    background: transparent;
    cursor: pointer;
  }
  /* El relleno llega al centro de la perilla (20 px): nunca asoma pista vacía debajo de ella. */
  input::-webkit-slider-runnable-track {
    height: 6px;
    border-radius: 99px;
    background:
      linear-gradient(var(--relleno), var(--relleno)) 0 0 / var(--lleno) 100% no-repeat,
      var(--superficie-hundida);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  input.apagado::-webkit-slider-runnable-track {
    background:
      linear-gradient(var(--texto-3), var(--texto-3)) 0 0 / var(--lleno) 100% no-repeat,
      var(--superficie-hundida);
  }
  input::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 20px;
    height: 20px;
    margin-top: -7px;
    border-radius: 50%;
    background: #ffffff;
    box-shadow:
      0 0 0 0.5px rgb(0 0 0 / 0.14),
      0 1px 2px rgb(0 0 0 / 0.2),
      0 3px 8px -2px rgb(0 0 0 / 0.28);
  }
  input:focus-visible {
    outline: none;
  }
  input:focus-visible::-webkit-slider-thumb {
    box-shadow: var(--foco);
  }
</style>
