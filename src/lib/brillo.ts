// Lo común del brillo: el tono (de ámbar tenue a amarillo intenso) y los pasos de − y +.
// Lo usan la tarjeta de cada pantalla y el widget de brillo.

const paradas: [number, [number, number, number]][] = [
  [0, [124, 74, 18]],
  [40, [217, 119, 6]],
  [75, [245, 158, 11]],
  [100, [250, 204, 21]],
];

/** El color del brillo: ámbar tenue abajo, amarillo intenso arriba. */
export function tonoDeBrillo(brillo: number): string {
  const b = Math.max(0, Math.min(100, brillo));
  for (let i = 1; i < paradas.length; i++) {
    const [b0, c0] = paradas[i - 1];
    const [b1, c1] = paradas[i];
    if (b <= b1) {
      const t = (b - b0) / (b1 - b0);
      const c = c0.map((v, k) => Math.round(v + (c1[k] - v) * t));
      return `rgb(${c[0]} ${c[1]} ${c[2]})`;
    }
  }
  return 'rgb(250 204 21)';
}

/** − y +: al múltiplo de 10 anterior o siguiente (31 → 30 → 20…; 31 → 40). */
export function pasoDeBrillo(actual: number, subir: boolean): number {
  const nuevo = subir ? (Math.floor(actual / 10) + 1) * 10 : Math.floor((actual - 1) / 10) * 10;
  return Math.max(0, Math.min(100, nuevo));
}
