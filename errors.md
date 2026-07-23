# Agregar Campo "Grupo de Empresa" a Usuarios

## Descripción

Los pagos ocurren dentro de un grupo de empresas (Ferreteria Principal, Pintatodo, Brink). Cuando se crea un usuario o cajera, debe especificarse a qué grupo pertenece. Aunque los correos de Binance no identifican a qué empresa van los pagos, el verificador necesita saber que el pago corresponde a su grupo al momento de verificar.

La solución es agregar un campo `company_group` al usuario. Este campo quedará visible en el perfil del verificador logueado y se mostrará en la pantalla de verificación de pagos, para que la cajera sepa que está verificando pagos de su empresa.

## Propuesta de Implementación

---

### Base de Datos

#### [MODIFY] [001_init.sql](file:///c:/Users/Usuario/Documents/binance-auditor1/server/migrations/001_init.sql)
- Agregar columna `company_group TEXT NULL CHECK (company_group IN ('ferreteria_principal', 'pintatodo', 'herramientas_brink'))` a la tabla `users`.
- Agregar columna `company_group TEXT NULL` a la tabla `payments` (se llena al verificar).

#### [NEW] [003_add_company_group.sql](file:///c:/Users/Usuario/Documents/binance-auditor1/server/migrations/003_add_company_group.sql)
- Migración que agrega `company_group` a `users` y a `payments`.

---

### Backend (Rust - Servidor)

#### [MODIFY] [models.rs](file:///c:/Users/Usuario/Documents/binance-auditor1/server/src/models.rs)
- Agregar campo `company_group: Option<String>` a `User` y `UserPublic`.
- Agregar `company_group` a `CreateUserRequest` y `UpdateUserRequest`.
- Agregar `company_group` al JWT `Claims` (opcional, para no hacer un lookup extra por request).

#### [MODIFY] [db.rs](file:///c:/Users/Usuario/Documents/binance-auditor1/server/src/db.rs)
- Incluir `company_group` en todas las queries `SELECT` de usuarios.
- Pasar `company_group` al `INSERT` en `create_user`.
- Incluir `company_group` en el `UPDATE` en `update_user`.
- En `verify_payment` y `quick_verify_payment`: recibir `company_group` del verificador y guardarlo en el pago.
- En `list_payments`: incluir `company_group` del pago en la respuesta.

#### [MODIFY] [routes/mod.rs](file:///c:/Users/Usuario/Documents/binance-auditor1/server/src/routes/mod.rs)
- Pasar `company_group` del request a `db::create_user`.
- Pasar `company_group` del request a `db::update_user`.
- En el login, incluir `company_group` en los datos del token JWT (claims).

#### [MODIFY] [auth.rs](file:///c:/Users/Usuario/Documents/binance-auditor1/server/src/auth.rs)
- Agregar `company_group: Option<String>` a `AuthUser` y `Claims`.

---

### Backend Tauri (Rust - Cliente)

#### [MODIFY] [http_client.rs](file:///c:/Users/Usuario/Documents/binance-auditor1/src-tauri/src/http_client.rs)
- Agregar `company_group: Option<&str>` al método `create_user`.
- Asegurarse que `update_user` pase el campo si viene en el `updates`.

#### [MODIFY] [main.rs](file:///c:/Users/Usuario/Documents/binance-auditor1/src-tauri/src/main.rs)
- Agregar `company_group: Option<String>` al comando `client_create_user` y pasarlo al http_client.

---

### Frontend (TypeScript/React)

#### [MODIFY] [auth.ts](file:///c:/Users/Usuario/Documents/binance-auditor1/src/types/auth.ts)
- Agregar `company_group: 'ferreteria_principal' | 'pintatodo' | 'brink' | null` a la interfaz `User`.

#### [MODIFY] [api.ts](file:///c:/Users/Usuario/Documents/binance-auditor1/src/lib/api.ts)
- Agregar `company_group?: string` al tipo de `createUser` y `updateUser`.
- Actualizar tipo de retorno de `listUsers` para incluir `company_group`.

#### [MODIFY] [Users.tsx](file:///c:/Users/Usuario/Documents/binance-auditor1/src/pages/Users.tsx)
- Agregar campo `company_group` a `UserRecord`.
- Agregar un `<select>` para seleccionar el grupo en el formulario de creación.
  - Opciones: Ferretería Principal, Pintatodo, Herramientas Brink (campo requerido para cashiers).
- Mostrar el grupo en la tabla de usuarios con un badge de color diferenciado por empresa.

#### [MODIFY] páginas de verificación y lista de pagos
- Mostrar badge del grupo en la columna de verificador en la tabla de pagos (`verified_by_name + company_group`).
- En la UI de verificación (Dashboard u otras), mostrar el `company_group` del usuario logueado para que la cajera sepa que está verificando pagos de su empresa.

---

## Diseño de Valores

| Valor en BD | Etiqueta Visible |
|---|---|
| `ferreteria_principal` | Ferretería Principal |
| `pintatodo` | Pintatodo |
| `herramientas_brink` | Herramientas Brink |

---

## Preguntas Abiertas

> [!IMPORTANT]
> **¿El campo es obligatorio?** ¿Se debe exigir que todo usuario/cajera tenga grupo asignado, o puede ser opcional (ej. para admins)?
> Se asume: **requerido para cashier, opcional para admin** (ya que los admins supervisan todo).

> [!NOTE]
> **Pagos y grupo de empresa:** Una vez verificado un pago, el `company_group` del verificador queda **registrado en el pago**. Así queda trazabilidad de qué empresa procesó cada pago.
> Los admins ven **todos** los pagos sin importar el grupo.

## Plan de Verificación

### Pruebas
- Crear migración y verificar que aplica sin errores.
- Crear un usuario cashier con grupo desde el formulario → aparece en tabla con badge correcto.
- Verificar que el campo se muestra en la pantalla de login/perfil del usuario logueado.
- Compilar el proyecto Rust (server y tauri) sin errores.

### Manual
- El admin crea una cajera con grupo "Pintatodo" → aparece en la lista.
- La cajera inicia sesión → en su UI de verificación puede ver que es de "Pintatodo".
