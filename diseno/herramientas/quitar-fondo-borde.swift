// Quita el fondo blanco solo donde toca el borde de la foto (relleno desde las orillas), así no se borran
// las partes claras de adentro, como la imagen en la pantalla de un monitor. Recorta lo transparente
// sobrante y deja la foto cuadrada con un margen chico.
// Uso: swift quitar-fondo-borde.swift entrada.(png|jpg) salida.png [--lado=640]
import AppKit

let args = CommandLine.arguments
guard args.count >= 3 else {
  fputs("Uso: swift quitar-fondo-borde.swift entrada salida.png [--lado=640]\n", stderr)
  exit(1)
}
let lado = args.first { $0.hasPrefix("--lado=") }.flatMap { Int($0.dropFirst(7)) } ?? 640

guard let imagen = NSImage(contentsOfFile: args[1]),
  let cg = imagen.cgImage(forProposedRect: nil, context: nil, hints: nil)
else { fputs("No pude abrir \(args[1])\n", stderr); exit(1) }

let w = cg.width, h = cg.height
var px = [UInt8](repeating: 0, count: w * h * 4)
let espacio = CGColorSpaceCreateDeviceRGB()
let ctx = CGContext(data: &px, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: espacio,
                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
ctx.draw(cg, in: CGRect(x: 0, y: 0, width: w, height: h))

// Casi blanco: fondo. Entre medio: borde suave (se le baja la opacidad).
@inline(__always) func claridad(_ i: Int) -> Int { min(Int(px[i]), Int(px[i + 1]), Int(px[i + 2])) }
let fondo = 236, suave = 200

var visto = [Bool](repeating: false, count: w * h)
var pila: [Int] = []
for x in 0..<w { pila.append(x); pila.append((h - 1) * w + x) }
for y in 0..<h { pila.append(y * w); pila.append(y * w + w - 1) }
while let p = pila.popLast() {
  if visto[p] { continue }
  let i = p * 4
  // Lo que ya es transparente también es fondo: se sigue avanzando por ahí (fotos con borde transparente).
  let transparente = px[i + 3] < 16
  let c = transparente ? 255 : claridad(i)
  if c < suave { continue }
  visto[p] = true
  if transparente || c >= fondo {
    px[i] = 0; px[i + 1] = 0; px[i + 2] = 0; px[i + 3] = 0
  } else {
    // Orilla: opacidad según lo lejos que está del blanco (colores premultiplicados).
    let a = Double(fondo - c) / Double(fondo - suave)
    for k in 0..<3 { px[i + k] = UInt8(Double(px[i + k]) * a) }
    px[i + 3] = UInt8(255 * a)
  }
  // Solo se sigue avanzando por lo que es fondo de verdad.
  if c < fondo { continue }
  let x = p % w, y = p / w
  if x > 0 { pila.append(p - 1) }
  if x < w - 1 { pila.append(p + 1) }
  if y > 0 { pila.append(p - w) }
  if y < h - 1 { pila.append(p + w) }
}

// Caja de lo visible, para recortar.
var x0 = w, y0 = h, x1 = 0, y1 = 0
for y in 0..<h { for x in 0..<w where px[(y * w + x) * 4 + 3] > 8 {
  x0 = min(x0, x); y0 = min(y0, y); x1 = max(x1, x); y1 = max(y1, y)
} }
let limpio = ctx.makeImage()!
let caja = CGRect(x: x0, y: y0, width: x1 - x0 + 1, height: y1 - y0 + 1)
let recorte = limpio.cropping(to: caja)!

// Cuadrado con 4 % de margen, al tamaño pedido.
let margen = Double(lado) * 0.04
let util = Double(lado) - margen * 2
let escala = util / Double(max(caja.width, caja.height))
let dw = Double(caja.width) * escala, dh = Double(caja.height) * escala
let salida = CGContext(data: nil, width: lado, height: lado, bitsPerComponent: 8, bytesPerRow: lado * 4, space: espacio,
                       bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
salida.interpolationQuality = .high
salida.draw(recorte, in: CGRect(x: (Double(lado) - dw) / 2, y: (Double(lado) - dh) / 2, width: dw, height: dh))
let rep = NSBitmapImageRep(cgImage: salida.makeImage()!)
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: args[2]))
print("Listo: \(args[2]) (\(lado)×\(lado) px)")
