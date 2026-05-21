local attrs = require"@taimi/pack/attrs"

local a = attrs.MarkerAttributes.new()

a:SetAttr("hi", 5)
a:SetAttr("AutoTrigger", true)

for k,v in pairs(a.attrs) do
	print(k, v)
end
