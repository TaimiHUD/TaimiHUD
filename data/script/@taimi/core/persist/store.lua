-- stub for testing; core modules are built-in to scripting engine
local Storage = {
	i = {}
}
Storage.mt = { __index = Storage.i }
function Storage:New(info, i)
	i = util.setmetatable(i or {}, self.mt)
	i.info = info
	if i.kv == nil then
		i.kv = {}
	end
	return i
end
function Storage.new(...)
	return Storage:New(...)
end

function Storage.i:InsertString(k, v)
	local prev = self:GetString(k)
	self.kv[k] = tostring(v)
	return prev
end
function Storage.i:GetString(k)
	local v = self.kv[k]
	if v ~= nil then
		v = tostring(v)
	end
	return v
end
function Storage.i:RemoveKey(k)
	self.kv[k] = nil
end

return {
	Storage = Storage,
}
