# Binance Pay Auditor - Instrucciones de Instalacion y Compilacion

## Que es esto

Una app de escritorio local para auditar y conciliar pagos de Binance basados en alertas de correo electronico. Se conecta a Gmail via IMAP, guarda los pagos en una base de datos SQLite local, y permite cruzar datos con reportes de la empresa.

## Requisitos Previos

### 1. Instalar Rust (OBLIGATORIO)

Tauri necesita Rust para compilar el ejecutable `.exe`.

1. Descarga el instalador desde: https://rustup.rs/
2. Ejecuta `rustup-init.exe` y acepta las opciones por defecto
3. **CIERRA Y REABRE la terminal** despues de instalar
4. Verifica con: `rustc --version` (debe mostrar algo como `rustc 1.xx.x`)

### 2. Instalar Build Tools de Windows

Para compilar en Windows necesitas el compilador de C++:

1. Descarga **Build Tools for Visual Studio 2022**: https://visualstudio.microsoft.com/visual-cpp-build-tools/
2. Ejecuta el instalador
3. Selecciona **"Desktop development with C++"** (solo eso, no necesitas todo Visual Studio)
4. Instala

### 3. Node.js (ya lo tenes instalado)

Ya tenes Node.js v22, asi que esto esta listo.

## Compilar el Ejecutable

### Modo desarrollo (para probar)

```bash
cd C:\Users\Usuario\Desktop\binance-auditor
npm install
npm run tauri:dev
```

Esto abre la app en modo desarrollo con hot-reload.

### Compilar el instalador (.exe)

```bash
cd C:\Users\Usuario\Desktop\binance-auditor
npm run tauri:build
```

Esto genera el instalador en:
```
C:\Users\Usuario\Desktop\binance-auditor\src-tauri\target\release\bundle\
```

Ahi vas a encontrar:
- `nsis/Binance Pay Auditor_1.0.0_x64-setup.exe` - Instalador NSIS (recomendado para distribuir)
- `msi/` - Instalador MSI alternativo

## Instalar en la PC de la Jefa de Tesoreria

1. Copia el archivo `.exe` del instalador a un USB o por red
2. En la PC destino, ejecuta el instalador
3. La app se instala como cualquier programa de Windows
4. Al abrirla, ir a **Ajustes** y configurar:
   - Correo de Gmail
   - Contrasena de aplicacion de Google (NO la contrasena normal)

### Como generar la Contrasena de Aplicacion de Google

1. Ir a https://myaccount.google.com/apppasswords
2. Iniciar sesion con la cuenta de Gmail que recibe las alertas de Binance
3. Crear una nueva contrasena de aplicacion
4. Copiar los 16 caracteres (formato: `abcd efgh ijkl mnop`)
5. Pegarla en la seccion **Ajustes** de la app

## Estructura del Proyecto

```
binance-auditor/
├── src/                          # Frontend React
│   ├── pages/
│   │   ├── Dashboard.tsx         # Pagina principal de conciliacion
│   │   ├── Reports.tsx           # Reportes por rango de fechas
│   │   └── Settings.tsx          # Configuracion de IMAP
│   ├── components/
│   │   ├── SyncButton.tsx        # Boton de sincronizacion de correos
│   │   ├── VerificationForm.tsx  # Formulario de verificacion cruzada
│   │   └── ReportTable.tsx       # Tabla de reportes
│   └── types/
│       └── index.ts              # Tipos TypeScript
├── src-tauri/                    # Backend Rust (Tauri)
│   ├── src/
│   │   ├── main.rs               # Entry point + Tauri commands
│   │   ├── db.rs                 # Base de datos SQLite
│   │   ├── imap.rs               # Sincronizacion IMAP de Gmail
│   │   └── settings.rs           # Persistencia de configuracion
│   ├── Cargo.toml                # Dependencias Rust
│   └── tauri.conf.json           # Configuracion de Tauri
└── package.json
```

## Notas Importantes

- **Base de datos**: SQLite, se guarda en `%APPDATA%\binance-auditor\auditoria.db`
- **Configuracion**: Se guarda en `%APPDATA%\binance-auditor\settings.json`
- **No requiere servidor**: Todo corre localmente en la PC
- **Si necesita internet**: Solo para leer correos de Gmail via IMAP
- **Los correos se marcan como leidos** despues de procesarlos para no duplicar
