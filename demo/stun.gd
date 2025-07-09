# AI #
extends RefCounted
class_name Stun

static func pub_addr() -> PubAddrAwaiter:
	return PubAddrAwaiter.new()

class PubAddrAwaiter:
	signal recv

	func _init():
		_run_stun()

	func _run_stun():
		var peer := PacketPeerUDP.new()
		var trans_id := _rand_id()
		var server = IP.resolve_hostname("stun.l.google.com")

		if server == "":
			recv.emit(null)
			return

		var req := PackedByteArray([0x00, 0x01, 0x00, 0x00, 0x21, 0x12, 0xA4, 0x42]) + trans_id
		peer.connect_to_host(server, 19302)
		peer.put_packet(req)

		var timer = Engine.get_main_loop().create_timer(2)
		timer.timeout.connect(func ():
			if peer.get_available_packet_count() > 0:
				var res = peer.get_packet()
				recv.emit(_parse(res, trans_id))
			else:
				recv.emit(null)
		)

	func _parse(data: PackedByteArray, _tid: PackedByteArray) -> Dictionary:
		var pos = 20
		while pos + 4 <= data.size():
			var attr_type = (data[pos] << 8) | data[pos + 1]
			var attr_len = (data[pos + 2] << 8) | data[pos + 3]
			pos += 4
			if pos + attr_len > data.size():
				break

			if attr_type == 0x0020 and attr_len >= 8:
				var fam = data[pos + 1]
				var xport = ((data[pos + 2] << 8) | data[pos + 3]) ^ 0x2112
				if fam == 0x01:
					var ipb = data.slice(pos + 4, pos + 8)
					for i in range(4):
						ipb[i] ^= [0x21, 0x12, 0xA4, 0x42][i]

					var ip_parts = [ipb[0], ipb[1], ipb[2], ipb[3]]
					return {
						"ip": "%d.%d.%d.%d" % ip_parts,
						"port": xport
					}
			pos += attr_len
			if attr_len % 4 != 0:
				pos += 4 - (attr_len % 4)
		return {}

	func _rand_id() -> PackedByteArray:
		var id := PackedByteArray()
		for i in range(12):
			id.append(randi() % 256)
		return id
