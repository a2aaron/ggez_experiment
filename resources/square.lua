SONGMAP = {}

-- "dump table" function from https://stackoverflow.com/a/27028488
-- modified slightly to add linebreaks
function dump(o)
   if type(o) == 'table' then
      local s = '{'
      for k,v in pairs(o) do
         if type(k) ~= 'number' then k = '"'..k..'"' end
         s = s .. '['..k..'] = ' .. dump(v) .. ','
      end
      return s .. '}\n'
   else
      return tostring(o)
   end
end

function add_action(beat, group, action)
    action["beat"] = beat
    action["enemygroup"] = group
    table.insert(SONGMAP, action)
end

function bullet(start_pos, end_pos)
    return {spawncmd = "bullet", start_pos = start_pos, end_pos = end_pos}
end

table.insert(SONGMAP, {bpm = 150.0})
table.insert(SONGMAP, {skip = 0.0 * 4.0})

add_action(1.0, 0, bullet(0.0, 0.0))


print(dump(SONGMAP))

return SONGMAP
