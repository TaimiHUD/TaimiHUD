-- TODO: move to core builtins and implement properly etc
local util = require"@taimi/util"
local lson = {
	encoders = {}
}

function lson.FromLson(enc)
	return assert(loadstring("return " .. enc, "=(lson)"))()
end

function lson.ToLson(v)
	local vty = type(v)
	local encoder = lson.encoders[vty]
	if encoder == nil then
		error(("cannot encode %s"):format(vty), 2)
	end
	return encoder(v)
end
function lson.encoders.table(v)
	local enc = nil
	local seqn = v[1] ~= nil
	if seqn then
		for i,elem in ipairs(v) do
			seqn = i
			if enc == nil then
				enc = "{" .. lson.ToLson(elem)
			else
				enc = enc .. "," .. lson.ToLson(elem)
			end
		end
	end
	for i,elem in pairs(v) do
		-- TODO: how to check if number is integer in lua
		local seen = seqn ~= nil and type(i) == "number" and i >= 1 and i <= seqn
		if not seen then
			if enc == nil then
				enc = "{"
			else
				enc = enc .. ","
			end
			enc = enc .. ("[%s]=%s"):format(lson.ToLson(i), lson.ToLson(elem))
		end
	end
	if enc == nil then
		return "{}"
	else
		return enc .. "}"
	end
end
-- function lson.encoders.userdata(v) return tostring(v) end
function lson.encoders.string(v)
	return ("%q"):format(v)
end
lson.encoders["nil"] = util.id("nil")
lson.encoders.number = tostring

return lson
