// Prepara la foto de un aparato para Nexo: fondo transparente, sin bordes sobrantes y cuadrada.
//
// Uso:
//   swift preparar-imagen.swift entrada.(png|jpg|webp) salida.png [--quitar-blanco] [--muestra=fondo.png]
//
//   --quitar-blanco   vuelve transparente el fondo blanco (y las marcas de agua claras),
//                     limpiando el borde blanco que queda alrededor del aparato.
//   --muestra=...     guarda además una vista sobre el fondo oscuro de Nexo, para revisarla.

import AppKit
import CoreGraphics

let args = CommandLine.arguments
guard args.count >= 3 else {
    fputs("Uso: swift preparar-imagen.swift entrada salida.png [--quitar-blanco] [--muestra=archivo.png]\n", stderr)
    exit(1)
}
let entrada = URL(fileURLWithPath: args[1])
let salida = URL(fileURLWithPath: args[2])
let quitarBlanco = args.contains("--quitar-blanco")
let muestra = args.first { $0.hasPrefix("--muestra=") }.map { URL(fileURLWithPath: String($0.dropFirst(10))) }

guard let origen = CGImageSourceCreateWithURL(entrada as CFURL, nil),
      let imagen = CGImageSourceCreateImageAtIndex(origen, 0, nil) else {
    fputs("No pude abrir \(entrada.path)\n", stderr); exit(1)
}

let ancho = imagen.width, alto = imagen.height
let espacio = CGColorSpace(name: CGColorSpace.sRGB)!
let info = CGImageAlphaInfo.premultipliedLast.rawValue
var px = [UInt8](repeating: 0, count: ancho * alto * 4)
px.withUnsafeMutableBytes { buf in
    let ctx = CGContext(data: buf.baseAddress, width: ancho, height: alto, bitsPerComponent: 8,
                        bytesPerRow: ancho * 4, space: espacio, bitmapInfo: info)!
    ctx.draw(imagen, in: CGRect(x: 0, y: 0, width: ancho, height: alto))
}

// 1) Fondo blanco → transparente. Opacidad según el canal más oscuro del píxel:
//    ≤ 180 queda opaco, ≥ 228 desaparece (fondos casi blancos y marcas de agua claras),
//    y en medio se suaviza el borde.
if quitarBlanco {
    let opaco = 180.0, transparente = 228.0
    let minimo = { (i: Int) in min(px[i], min(px[i + 1], px[i + 2])) }

    // Un gris que no toca nada oscuro (a 8 px) no es parte del aparato: es fondo o marca de agua.
    var oscuro = [Bool](repeating: false, count: ancho * alto)
    for p in 0..<(ancho * alto) { oscuro[p] = minimo(p * 4) < 90 }
    func tocaOscuro(_ x: Int, _ y: Int) -> Bool {
        for dy in -8...8 { for dx in -8...8 {
            let nx = x + dx, ny = y + dy
            if nx >= 0, ny >= 0, nx < ancho, ny < alto, oscuro[ny * ancho + nx] { return true }
        } }
        return false
    }

    for p in 0..<(ancho * alto) where px[p * 4 + 3] == 255 {
        let i = p * 4
        if minimo(i) >= 120 && !tocaOscuro(p % ancho, p / ancho) {
            px[i] = 0; px[i + 1] = 0; px[i + 2] = 0; px[i + 3] = 0
            continue
        }
        let r = Double(px[i]), g = Double(px[i + 1]), b = Double(px[i + 2])
        let a = max(0, min(1, (transparente - min(r, g, b)) / (transparente - opaco)))
        // Color premultiplicado sin el blanco mezclado: c·a = c_original − (1 − a)·255
        let limpio = { (c: Double) in UInt8(max(0, min(255, (c - (1 - a) * 255).rounded()))) }
        px[i] = limpio(r); px[i + 1] = limpio(g); px[i + 2] = limpio(b)
        px[i + 3] = UInt8((a * 255).rounded())
    }
}

// 2) Recorte al contenido visible.
var minX = ancho, minY = alto, maxX = -1, maxY = -1
for y in 0..<alto {
    for x in 0..<ancho where px[(y * ancho + x) * 4 + 3] > 12 {
        minX = min(minX, x); maxX = max(maxX, x); minY = min(minY, y); maxY = max(maxY, y)
    }
}
guard maxX >= 0 else { fputs("La imagen quedó vacía\n", stderr); exit(1) }
let recorteAncho = maxX - minX + 1, recorteAlto = maxY - minY + 1

let completa = px.withUnsafeMutableBytes { buf -> CGImage in
    let ctx = CGContext(data: buf.baseAddress, width: ancho, height: alto, bitsPerComponent: 8,
                        bytesPerRow: ancho * 4, space: espacio, bitmapInfo: info)!
    return ctx.makeImage()!
}
// CGImage cuenta y desde arriba; el contexto desde abajo.
let recorte = completa.cropping(to: CGRect(x: minX, y: minY, width: recorteAncho, height: recorteAlto))!

// 3) Lienzo cuadrado con un pequeño margen, centrado.
let lado = Int(Double(max(recorteAncho, recorteAlto)) * 1.06)
func lienzo(fondo: CGColor?) -> CGImage {
    let ctx = CGContext(data: nil, width: lado, height: lado, bitsPerComponent: 8, bytesPerRow: 0,
                        space: espacio, bitmapInfo: info)!
    if let fondo { ctx.setFillColor(fondo); ctx.fill(CGRect(x: 0, y: 0, width: lado, height: lado)) }
    ctx.interpolationQuality = .high
    ctx.draw(recorte, in: CGRect(x: (lado - recorteAncho) / 2, y: (lado - recorteAlto) / 2,
                                 width: recorteAncho, height: recorteAlto))
    return ctx.makeImage()!
}

func guardar(_ img: CGImage, en url: URL) {
    let png = NSBitmapImageRep(cgImage: img).representation(using: .png, properties: [:])!
    try! png.write(to: url)
}

guardar(lienzo(fondo: nil), en: salida)
if let muestra { guardar(lienzo(fondo: CGColor(srgbRed: 0.08, green: 0.1, blue: 0.13, alpha: 1)), en: muestra) }
print("Listo: \(salida.lastPathComponent) (\(lado)×\(lado) px)")
