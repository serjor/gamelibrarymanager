# 🎯 Lo que el webview necesita del entorno se pone en `main.rs`, no en el script

## 💡 Convention

Cuando el webview necesita una variable de entorno para funcionar en una
plataforma, esa variable se pone en
[`src-tauri/src/main.rs`](../../src-tauri/src/main.rs), en las primeras líneas
de `main()` y **antes de que se inicialice GTK**. No en `package.json`, no en el
script de desarrollo, no en el perfil de la shell de quien programa.

Tres reglas más, que son las que hacen que esto no se convierta en un cajón:

- **Detrás de `#[cfg(target_os = ...)]`.** Un apaño de Linux no se ejecuta en
  Windows ni en macOS, donde el motor del webview es otro.
- **Se respeta lo que ya venga del entorno.** Se comprueba con `var_os` antes de
  escribir, para que quien quiera el comportamiento original pueda pedirlo sin
  recompilar.
- **Se anota la vigencia con fecha, versión y síntoma**, como con los endpoints
  no oficiales. Estos apaños existen por un fallo de un motor concreto con un
  driver concreto: son deuda con fecha de caducidad, y sin el síntoma escrito
  nadie sabrá nunca si ya se puede quitar.

## 🏆 Benefits

- **Vale también para la aplicación empaquetada.** Un apaño en el script de
  desarrollo solo arregla la máquina de quien la programa. El usuario que se
  descarga el binario se encuentra una ventana que no abre, y una ventana que no
  abre no se depura: se desinstala.
- **No hay nada que recordar.** Ni una variable en el perfil, ni un `README` con
  un «si usas Wayland, ejecuta esto en vez de aquello».
- **El motivo vive al lado del código.** El día que WebKitGTK lo arregle, el
  comentario dice qué probar para saber si ya sobra.
- **Es portable sin dependencias.** Un prefijo de variable delante de un comando
  no es sintaxis válida en `cmd.exe`, así que ponerlo en `package.json` acaba
  costando un paquete más solo para arrancar.

## 👀 Examples

### ✅ Good

```rust
/// Vigencia: comprobado el 2026-08-15 en KDE sobre Wayland con `webkit2gtk-4.1`
/// 2.52.5 y una NVIDIA RTX 5070 Ti. Sin la variable, el binario sale con código
/// 1 y `Gdk-Message: Error 71 (Error de protocolo)`; con ella, la ventana abre.
#[cfg(target_os = "linux")]
fn sortear_dmabuf_de_webkit() {
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    sortear_dmabuf_de_webkit();

    gamelibrarymanager_lib::run()
}
```

### ❌ Bad

```json
{
  "scripts": {
    "tauri": "WEBKIT_DISABLE_DMABUF_RENDERER=1 tauri"
  }
}
```

Arregla exactamente una máquina: la de quien lo escribió. El binario que se
distribuye no pasa por ese script, así que el usuario con una tarjeta NVIDIA
sobre Wayland sigue viendo cómo la aplicación se cierra sola al abrirla, sin
ningún mensaje en ninguna ventana. Y el prefijo no funciona en Windows, así que
el arreglo de un sistema rompe el arranque en otro.

## 🧐 Real world examples

- [`src-tauri/src/main.rs`](../../src-tauri/src/main.rs) apaga el renderizador
  DMA-BUF de WebKitGTK antes de arrancar, solo en Linux y solo si el entorno no
  dice otra cosa, con el síntoma exacto y la fecha en que se comprobó.

## 🔗 Related agreements

- [Los endpoints no oficiales se contrastan antes de escribir el conector](../connectors/contrastar-endpoints-no-oficiales.md)
  — de ahí sale la idea de anotar la vigencia con fecha: es la misma clase de
  deuda, apoyada en algo que no controlamos y que cambiará.
- [Todo enlace de la interfaz necesita alcance explícito en la capacidad](alcance-de-urls-en-capacidades.md)
  — la otra cosa que solo se descubre ejecutando la aplicación de verdad.
- [Una comprobación afirma sobre la estructura, no sobre lo que parece](../testing/afirmar-sobre-la-estructura.md)
  — este apaño se eligió comparando el binario con la variable y sin ella, no
  suponiendo cuál era la causa.
