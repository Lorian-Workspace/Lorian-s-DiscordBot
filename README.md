# 🤖 Lorian Discord Bot

Un bot de Discord de alto rendimiento desarrollado en **Rust** usando el framework **Serenity**.

## ✨ Características

- **Alto rendimiento**: Desarrollado en Rust para máxima eficiencia
- **Baja latencia**: Respuesta rápida a comandos
- **Comandos slash**: Soporte para comandos modernos de Discord (no usa prefijos)
- **Logging avanzado**: Sistema de logs detallado
- **Configuración flexible**: Variables de entorno y archivos de configuración
- **Arquitectura modular**: Código organizado y fácil de extender
- **Persistencia de datos**: Guarda información relevante (tickets, feedback, contexto de chat, etc.) en archivos locales para sobrevivir reinicios

## 🚀 Instalación

### Prerrequisitos

- [Rust](https://rustup.rs/) (versión 1.70 o superior)
- [Git](https://git-scm.com/)

### Pasos de instalación

1. **Clonar el repositorio**
   ```bash
   git clone https://github.com/tu-usuario/lorian-discord-bot.git
   cd lorian-discord-bot
   ```

2. **Configurar el bot**
   - Ve a [Discord Developer Portal](https://discord.com/developers/applications)
   - Crea una nueva aplicación
   - Ve a la sección "Bot" y crea un bot
   - Copia el token del bot

3. **Configurar variables de entorno**
   ```bash
   # Crear archivo .env
   echo "DISCORD_TOKEN=tu_token_aqui" > .env
   echo "SERVER_INVITE_URL=https://discord.gg/tu_codigo_estable" >> .env
   echo "VERIFICATION_CHANNEL_ID=tu_canal_de_verificacion" >> .env
   echo "UNVERIFIED_ROLE_ID=tu_rol_no_verificado" >> .env
   echo "VERIFIED_ROLE_ID=tu_rol_verificado" >> .env
   echo "SUBSCRIBER_ROLE_ID=tu_rol_suscriptor_de_anuncios" >> .env
   echo "RUST_LOG=info" >> .env
   ```

4. **Compilar y ejecutar**
   ```bash
   # Compilar en modo debug
   cargo build

   # Ejecutar
   cargo run

   # O compilar en modo release para mejor rendimiento
   cargo build --release
   cargo run --release
   ```

## 📋 Comandos

### Comandos slash (recomendados)
- `/ping` - Responde con "Pong!"
- `/info` - Información del bot en formato embed
- `/hola` - Saluda al usuario
- `/help` - Muestra ayuda de los comandos

## 🏗️ Estructura del proyecto

```
src/
├── main.rs          # Punto de entrada principal
├── commands/        # Módulo de comandos
│   ├── mod.rs
│   ├── ping.rs      # Comando ping
│   ├── info.rs      # Comando info
│   └── help.rs      # Comando help
├── events/          # Manejadores de eventos
│   ├── mod.rs
│   ├── ready.rs     # Evento de conexión
│   └── message.rs   # Evento de mensajes
├── utils/           # Utilidades
│   ├── mod.rs
│   ├── config.rs    # Configuración
│   ├── logger.rs    # Sistema de logging
│   └── storage.rs   # Persistencia de datos
```

## ⚙️ Configuración

### Variables de entorno

| Variable         | Descripción                        | Por defecto |
|------------------|------------------------------------|-------------|
| `DISCORD_TOKEN` | Token del bot de Discord | Requerido |
| `SERVER_INVITE_URL` | Invitación estable existente (`https://discord.gg/...` o `https://discord.com/invite/...`) | Requerido |
| `VERIFICATION_CHANNEL_ID` | Canal del panel canónico de verificación/suscripción | Requerido |
| `UNVERIFIED_ROLE_ID` | Rol aplicado a miembros que entren después del cutoff | Requerido |
| `VERIFIED_ROLE_ID` | Rol aplicado al verificar | Requerido |
| `SUBSCRIBER_ROLE_ID` | Rol separado para la suscripción opcional a DMs de anuncios | Requerido |
| `RUST_LOG` | Nivel de logging | `info` |

El owner ID está hardcodeado en `src/config.rs` (`OWNER_ID`). Los cinco IDs/URL de seguridad son fail-closed: si faltan o son inválidos, el bot no inicia; si Discord no permite validar canales, roles, jerarquía o permisos durante `ready`, toda la función de seguridad queda desactivada. Los canales fijos son:

- Anuncios: `1400467682440118333`.
- Barrera/honeypot: `1526610057511567380`.

`SERVER_INVITE_URL` debe apuntar a una invitación estable ya administrada. El bot no crea invitaciones durante incidentes.

### Verificación y suscripción opcional

En la primera activación válida, el bot persiste `verification_started_at`. Miembros con `joined_at` anterior quedan grandfathered: no reciben roles ni DMs automáticamente. Miembros posteriores reciben `UNVERIFIED_ROLE_ID`, sin DM. El panel canónico contiene:

- **Verify**: añade `VERIFIED_ROLE_ID` antes de quitar `UNVERIFIED_ROLE_ID`; ante un fallo, el flujo se cierra sin kick y responde de forma explícita.
- **Not now**: no cambia roles, no expulsa y no afecta la membresía.
- **Subscribe/Unsubscribe**: preferencia opcional, separada de la verificación. Una entrega requiere simultáneamente ledger activo, `SUBSCRIBER_ROLE_ID`, membresía humana actual y `VERIFIED_ROLE_ID` o grandfathering. Unsubscribe suprime primero el ledger, por lo que un fallo al quitar el rol no vuelve al usuario elegible.

Los roles de Discord son la fuente durable de verdad para verificación. El JSON sólo guarda cutoff, mensajes canónicos, preferencias, dedupe y recuperación necesaria. Reinicios reconcilian entradas posteriores al cutoff que llegaron mientras el bot estaba offline y eliminan registros de usuarios que ya salieron.

Configura manualmente los overwrites de `UNVERIFIED_ROLE_ID`: permitir ver/usar `VERIFICATION_CHANNEL_ID` y negar los canales protegidos que correspondan. El bot valida y avisa, pero nunca reescribe permisos de todo el servidor.

### Permisos del bot

Asegúrate de que tu bot tenga los siguientes permisos:
- Send Messages
- Use Slash Commands
- Read Message History
- Embed Links
- Manage Roles
- Ban Members
- Manage Messages

La jerarquía debe ser: rol más alto del bot por encima de `UNVERIFIED_ROLE_ID`, `VERIFIED_ROLE_ID` y `SUBSCRIBER_ROLE_ID`. Los tres roles deben ser distintos y no administrados por integraciones.

Activa estos Gateway Intents:

- `GUILDS`
- `GUILD_MEMBERS` (privileged; joins y recuperación posterior al cutoff)
- `GUILD_MODERATION` (recuperación ban/unban)
- `GUILD_MESSAGES`
- `GUILD_MESSAGE_REACTIONS`
- `MESSAGE_CONTENT` (privileged; usado por el contenido del anuncio autorizado; no se enumeran miembros para anunciar)

### Anuncios y barrera de seguridad

Sólo un mensaje humano, no-webhook, del owner hardcodeado (`OWNER_ID` en `src/config.rs`) en `1400467682440118333` inicia un anuncio. El bot toma una instantánea de suscriptores elegibles, revalida ledger/rol/membresía antes de cada DM, excluye bots y autor, desactiva menciones, envía secuencialmente y persiste dedupe por mensaje fuente. Serenity gestiona rate limits; no hay sleeps fijos.

El canal `1526610057511567380` mantiene exactamente un aviso canónico en seis idiomas. Ese aviso declara que publicar allí provoca un DM de seguridad con la invitación estable, un ban temporal con `delete_message_seconds=172800` y un unban inmediato. Fallo de ban no intenta unban; fallo de unban queda marcado como CRITICAL y se recupera en `ready`. La imagen semántica `safety.barrier` está en `bot_images.toml`; URL ausente/inválida omite la imagen sin detener el aviso.

### Checklist de staging (sin afirmar pruebas live)

1. Crear los tres roles distintos y colocar el rol del bot por encima de ellos.
2. Configurar overwrites de Unverified sólo en los canales deseados; no bloquear el panel de verificación.
3. Dar permisos/intents anteriores y comprobar que `MESSAGE_CONTENT` está habilitado en Developer Portal.
4. Configurar las siete variables requeridas y arrancar en un guild de staging.
5. Verificar que queda un solo panel en `VERIFICATION_CHANNEL_ID` y un solo aviso en el honeypot tras dos reconexiones.
6. Probar miembro anterior al cutoff (sin cambio) y posterior (Unverified), Verify, Not now, Subscribe y Unsubscribe.
7. Probar anuncio con owner/no-owner/webhook; confirmar sólo la intersección elegible y ningún ping.
8. Probar barrera con una cuenta desechable; revisar audit log, borrado de 48 horas, unban y recuperación tras reinicio simulado.

## 🔧 Desarrollo

### Agregar nuevos comandos

1. Crea un nuevo archivo en `src/commands/`
2. Implementa la función del comando
3. Registra el comando en `src/commands/mod.rs`
4. Agrega el comando al grupo en `src/main.rs`

Ejemplo de comando:
```rust
use serenity::builder::CreateApplicationCommand;

pub fn register(cmd: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    cmd.name("ejemplo").description("Un comando de ejemplo")
}
```

### Agregar nuevos eventos

1. Crea un nuevo archivo en `src/events/`
2. Implementa el manejador del evento
3. Registra el evento en `src/events/mod.rs`

## 📊 Rendimiento

Este bot está optimizado para:

- **Baja latencia**: Respuesta rápida a comandos
- **Bajo uso de memoria**: Sin garbage collector
- **Alta concurrencia**: Manejo eficiente de múltiples conexiones
- **Compilación optimizada**: Configuración de release para máximo rendimiento

## 🛠️ Tecnologías utilizadas

- **Rust**: Lenguaje principal
- **Serenity**: Framework para Discord API
- **Tokio**: Runtime asíncrono
- **Tracing**: Sistema de logging
- **Anyhow**: Manejo de errores
- **Serde**: Serialización

## 📝 Licencia

Este proyecto está bajo la Licencia MIT. Ver el archivo [LICENSE](LICENSE) para más detalles.
## 🔄 Auto-Update

El bot incluye un sistema de auto-actualización para Linux x86_64:

- **Activación**: Habilitado por defecto en builds de release
- **Intervalo**: Verifica cada 6 horas
- **Kill switch**: `AUTO_UPDATE_ENABLED=false`
- **Manual**: `/update` (solo owner)
- **Trust root**: GitHub releases + SHA256 checksum (no publisher signature)
- **Requisitos**: Directorio ejecutable con permisos de escritura

**Nota importante**:
- La primera actualización debe ser manual (bootstrap)
- No hay rollback automático después de crash
- Si el nuevo binario falla, restaurar manualmente desde `.bak`
- El estado pending se limpia después de Discord ready
- Proteger tags `v*` con ruleset antes del primer release


## 🤝 Contribuciones

Las contribuciones son bienvenidas. Por favor:

1. Fork el proyecto
2. Crea una rama para tu feature (`git checkout -b feature/AmazingFeature`)
3. Commit tus cambios (`git commit -m 'Add some AmazingFeature'`)
4. Push a la rama (`git push origin feature/AmazingFeature`)
5. Abre un Pull Request

## 📞 Soporte

Si tienes problemas o preguntas:

1. Revisa los [Issues](https://github.com/tu-usuario/lorian-discord-bot/issues)
2. Crea un nuevo issue si no encuentras una solución
3. Únete a nuestro servidor de Discord para soporte en tiempo real

---

**¡Disfruta programando con Rust! 🦀**
