local attrs = require"@taimi/pack/attrs"

local a = attrs.MarkerAttributes.new()

a:SetAttr("hi", 5)
a:SetAttr("AutoTrigger", true)

for k,v in pairs(a.attrs) do
	print(k, v)
end

local lson = require"@taimi/todo/lson"
local idk = lson.ToLson({
	"a",
	"b",
	["c"] = "zzz",
	["d"] = 3,
})
print(idk)
local backagain = lson.FromLson(idk)
print(backagain)
local idk = lson.ToLson(backagain)
print(idk)
