<!--
  Un aparato de Tuya encontrado en tu red: solo funciona con la nube de Tuya. Lo que se puede hacer, sin apuro
  (en "Buscar aparatos" y en "Tu red", al tocar "Ver opciones"):
  - Si tiene Matter, Nexo lo manejará más adelante.
  - Con firmware abierto (OpenBeken o Tasmota), Nexo lo maneja directo en tu red.
  - O una cuenta de Tuya como extra opcional: lleva a Ajustes › Casa con "Otras cuentas" abierta.
-->
<script lang="ts">
  let { id, onir }: { id?: string; onir: (seccion: string, id?: string) => void } = $props();

  function conectarTuya() {
    try {
      sessionStorage.setItem('nexo-abrir-tuya', '1');
    } catch {
      // Sin almacenamiento: "Otras cuentas" se abre a mano.
    }
    onir('ajustes', 'casa');
  }
</script>

<ul class="opciones" {id}>
  <li><b>Si tiene Matter</b> (el logo está en su caja o en su app): Nexo podrá manejarlo más adelante.</li>
  <li><b>Con firmware abierto</b> (OpenBeken o Tasmota): Nexo lo maneja directo, en tu red.</li>
  <li>
    <span><b>O conecta una cuenta de Tuya</b>, como extra opcional.</span>
    <button type="button" class="enlace" onclick={conectarTuya}>Abrir Ajustes</button>
  </li>
</ul>

<style>
  .opciones {
    display: grid;
    gap: 6px;
    margin: 0;
    padding: 10px 12px 10px 30px;
    border-radius: var(--radio-sm);
    background: var(--superficie);
    box-shadow: inset 0 0 0 1px var(--borde);
    font: var(--letra-pequena);
    color: var(--texto-2);
  }
  li {
    overflow-wrap: break-word;
  }
  li::marker {
    color: var(--texto-3);
  }
  b {
    font-weight: 600;
    color: var(--texto);
  }
  .enlace {
    margin-left: 6px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--primario-texto);
    font: inherit;
    font-weight: 600;
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .enlace:hover {
    text-decoration-thickness: 2px;
  }
</style>
