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
   echo "OWNER_ID=1400464001133056111" >> .env
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
| `DISCORD_TOKEN`  | Token del bot de Discord           | Requerido   |
| `OWNER_ID`       | ID del owner del Discord           | Requerido   |
| `RUST_LOG`       | Nivel de logging                   | `info`      |

### Permisos del bot

Asegúrate de que tu bot tenga los siguientes permisos:
- Send Messages
- Use Slash Commands
- Read Message History
- Embed Links

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