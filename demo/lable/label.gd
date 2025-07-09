extends Control

@onready var label: Label = $HBoxContainer/Label

var l: GodotListing
signal join(id: String)

func listing(new_l: GodotListing) -> void:
	l = new_l
	label.text = l.listing_no_id.name


func _on_button_pressed() -> void:
	join.emit(l.id)
