-- stub for testing; core modules are built-in to scripting engine
local vectors = {}

function vectors.Vec3(x, y, z)
	return {
		X = x,
		Y = y,
		Z = z,
	}
end

function vectors.Colour(r, g, b, a)
	return {
		R = r,
		G = g,
		B = b,
		A = a or 255,
	}
end

function vectors.Guid(guid)
	return {
		guid = guid,
		ToBase64 = function(g) return g.guid end,
	}
end

return vectors
