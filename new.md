# Lorian-s-DiscordBot

variables: OWNER DEL DISCORD : "1400464001133056111"

Commandos:

para los comandos usamos el "/" de discord que es para integrar apps, nada de prefixes y esas cosas como "!" etc.

agreguemos un commando help.


Habilidad de datos:


mensajes de diferentes systemas como feedback, Ticket Support, Sistema de Comision, necesitamos guardar los datos de la id de mensajes como los mensajes del bot, osea ejemplo el mensaje para creat ticket cuando el bot se reinicia necesita saber en que botton pues hara tal accion y tambien los botones de los canales creados por tickets para poder cerrar los tickets incluso si el bot se reinicio, osea la data de la ram en un archivo local.

esto tambien para guardar datos de hablar con el bot, digamos que estamos hablando con la ia, esta tiene contexto digamos de 15 mensajes anteriores necesitamos guardar este contexto para cuando hablemos con el usuario otra ves pues este tenga en cuenta los ultimos 15 mensajes con este usuario y tambien los 15 mensajes de la conversacion actual del chat, osea de otros usuarios talves hablando entre ellos o asi.

habilidades:

1. hablar en el canal con la id : "1400493466080903171"
atraves de ia, como si fuera un chatgpt, usando la apikey de gemini, sus mensajes estan en embeds
teniendo emociones con imagenes de estas.

este recomienda proyectos, puede crear los embeds de su mensaje con bottones, ejemplo si necesitas ayuda en un trabajo de programacion
y este ve que necesitas un dessarrollador, o ilustrador  pues este crea un botton el cual es como COmisioname, o cosa del estilo

este tiene en su prompt informacion de thelorian para poder hablar con info.

2. Ticket SUpport, en el canal "1400493422036648088" hay un mensaje embed enviado por este en el cual tiene un botton de discord que al darle click te agrega a un canal personal que tiene permiso el que habrio el ticket y el bot, en el cual el bot le resuelve dudas pero tambien tagea al owner del server de discord.

Al crearse el canal de ticket el bot instantaneamente manda un mensaje embed en este que tiene el contenido y bottones que solo puede clicar el owner pero tambien el que lo a abierto como Cerrar Ticket.

3. Un sistema de comisiones es lo mismo que crear tickets pero este no es para problemas sino para comisiones haciendo que tienes acceso al correo de thelorian como theloria@centaury.net, y puedes hablar con este respecto a negocios, comisiones y ideas y demas.
El canal de comisiones es "1400493436993278043" y  al crear el ticket de comision entonces se crea un canal, oculto para todos etc.


4. Feedback system, basicamente cuando las personas mandan un mensaje en "1400466972293992498" entonces el bot detecta si tiene malas palabras con una lib, sino los borra, y pues cuando el usuario crea el mensaje este borra el mensaje del usuario y lo re envia pero en formato embed con en el embed el autor seria el usuario nombre, el mensaje que dio, el embed color morado claro y el bot reacciona al embed con el emoji que es up y tambien down, en el mensaje embed hay un lugar donde van como el emoji Star que serian 5 estrellas y si no va una estrella entonces se cambia esta por el emoji de "X"

basicamente se hace una media digamos tenemos 5 votos a favor con el emoji reaccionado y 5 a contra entonces serian 2,5 valor estrellas pero arredondamos para abajo entonces serian 2 estrellas de emoji y lo demas son 3 emojis de x, comprendes ? basicamente actualiza el embed.