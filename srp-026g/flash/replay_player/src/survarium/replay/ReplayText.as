package survarium.replay
{
   import survarium.constants.TeamSide;

   // The exe's one-string forms of the root's lists (set_markers_text, set_players_text): GFx's
   // Invoke passes a string more easily than an Array of objects. Entries are joined by ";", their
   // fields by ":" (the last field, a text, is the rest of the entry: it may hold ":", never ";");
   // a malformed entry is left out, an empty string is an empty list.
   public class ReplayText
   {
      // `time_ms:kind:label;...` (kind 1 a kill, 2 an objective; the label may be empty): the
      // markers [{time, kind, label}] in time order, as set_markers_list takes them (the prev/next
      // label buttons then step from one to the next).
      public static function markers(value:String) : Array
      {
         var list:Array = [];
         for each(var entry:String in ReplayText.entries(value))
         {
            var f:Array = ReplayText.fields(entry);
            if(f != null && ReplayText.isNumber(f[0]) && ReplayText.isNumber(f[1]))
            {
               list.push({
                  "time":uint(f[0]),
                  "kind":uint(f[1]),
                  "label":String(f[2])
               });
            }
         }
         list.sortOn("time",Array.NUMERIC);
         return list;
      }

      // `place:team:name;...` (place 0..19, what select_player sends back; team the match's 0 or 1,
      // shown as ally and enemy): the players [{id, team, name}], as set_players_list takes them.
      public static function players(value:String) : Array
      {
         var list:Array = [];
         for each(var entry:String in ReplayText.entries(value))
         {
            var f:Array = ReplayText.fields(entry);
            if(f != null && ReplayText.isNumber(f[0]) && uint(f[0]) <= 19 && (f[1] == "0" || f[1] == "1") && f[2].length > 0)
            {
               list.push({
                  "id":uint(f[0]),
                  "team":f[1] == "0" ? TeamSide.ALLY : TeamSide.ENEMY,
                  "name":String(f[2])
               });
            }
         }
         return list;
      }

      private static function entries(value:String) : Array
      {
         return value == null || value.length == 0 ? [] : value.split(";");
      }

      // [first, second, the rest] of an entry, null if it has fewer than 3 fields.
      private static function fields(entry:String) : Array
      {
         var one:int = entry.indexOf(":");
         var two:int = one < 0 ? -1 : entry.indexOf(":",one + 1);
         return two < 0 ? null : [entry.substring(0,one),entry.substring(one + 1,two),entry.substring(two + 1)];
      }

      private static function isNumber(text:String) : Boolean
      {
         return /^[0-9]{1,10}$/.test(text) && Number(text) <= uint.MAX_VALUE;
      }
   }
}
