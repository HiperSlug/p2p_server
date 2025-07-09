extends Node

var hosting: bool = false

var addr: Dictionary = {}
@onready var client: PunchingClient = $PunchingClient

func _ready() -> void:
	signals()
	
	addr = await Stun.pub_addr().recv
	
	client.connect("127.0.0.1", 3000, addr.ip, addr.port) 
	print(await client.connection_changed)

func signals():
	client.async_error.connect(async_error)
	client.connection_changed.connect(on_connection_changed)
	client.joined_addrs_changed.connect(joined)
	client.owned_listing_changed.connect(owned)
	client.listings_changed.connect(on_listings)

func async_error(msg: String) -> void:
	printerr("Async: ", msg)

func on_connection_changed(new_connection):
	print("status: ", new_connection)

func joined(addrs):
	print(addrs)

func owned(id):
	print(id)

func on_listings(arr):
	print(arr)
	
	for c in cont.get_children():
		c.queue_free()
	
	for l in arr:
		var label = LABEL.instantiate()
		
		cont.add_child(label)
		label.listing(l)
		label.join.connect(join)



func _on_host_pressed() -> void:
	var listing = GodotListingNoId.new()
	listing.name = "Test1"
	client.create_listing(listing)
	hosting = true



@onready var cont: VBoxContainer = $VBoxContainer2
const LABEL = preload("res://lable/label.tscn")
func _on_refresh_pressed() -> void:
	
	
	client.get_listings()


func join(addr: String):
	if hosting:
		as_server()
	else:
		as_client(addr)


func as_server():
	var peer = WebSocketMultiplayerPeer.new()
	peer.create_server(443)
	multiplayer.multiplayer_peer = peer

func as_client(addr: String):
	var peer = WebSocketMultiplayerPeer.new()
	peer.create_client("ws://" + addr)
	multiplayer.multiplayer_peer = peer
