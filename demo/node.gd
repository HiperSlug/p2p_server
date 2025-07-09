extends Node

var addr: Dictionary = {}
@onready var client: PunchingClient = $PunchingClient

func _ready() -> void:
	client.async_error.connect(_on_punching_client_async_error)
	
	addr = await Stun.pub_addr().recv
	print(addr)
	client.connect("127.0.0.1", 3000, addr.ip, addr.port) 
	print(await client.connection_changed)


func _on_button_pressed() -> void:
	pass # Replace with function body.



func _on_punching_client_async_error(msg: String) -> void:
	printerr("Error: ", msg)
