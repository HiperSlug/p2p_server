extends Node

var addr: Dictionary = {}
var client := PunchingClient.new()

func _ready() -> void:
	addr = await Stun.pub_addr().recv
	print(addr)
	client.connect("127.0.0.1", 3000, addr.ip, addr.port)
	print(await client.connection_changed)


func _on_button_pressed() -> void:
	pass # Replace with function body.


func _on_button_2_pressed() -> void:
	pass # Replace with function body.
