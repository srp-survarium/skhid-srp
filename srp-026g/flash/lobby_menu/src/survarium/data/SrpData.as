package survarium.data
{
   // srp's tables as the game's configs have them, gameplay/srp/<table>.options (made by
   // srp-026g/scripts/content-edit/<table>.tsv, which srp's server reads too): exe patch 94 reads
   // each when the game starts and gives it to this movie, root.srp_data(table, key, value, ...),
   // in parts. A key's "/" is a table and "[i]" an item of a list, as in the config; a value is a
   // Number when it reads as one. The movie keeps no copy of a value: it reads it here.
   public class SrpData
   {
      private static var tables:Object = {};

      // A part of a table: its name, then its keys and values.
      public static function take(values:Array) : void
      {
         var name:String = String(values[0]);
         if(tables[name] == null)
         {
            tables[name] = {};
         }
         for(var i:int = 1; i + 1 < values.length; i += 2)
         {
            put(tables[name],String(values[i]),String(values[i + 1]));
         }
      }

      // The rooms' and Player vs Bots' settings (rooms.tsv): size_least, waits, pvb.sizes, ...
      public static function get rooms() : Object
      {
         return tables.rooms;
      }

      // "pvb/sizes[1]": table pvb, its list sizes, item 1.
      private static function put(table:Object, key:String, text:String) : void
      {
         var steps:Array = [];
         for each(var part:String in key.split("/"))
         {
            var open:int = part.indexOf("[");
            steps.push(open < 0 ? part : part.substring(0,open));
            while(open >= 0)
            {
               var close:int = part.indexOf("]",open);
               steps.push(int(part.substring(open + 1,close)));
               open = part.indexOf("[",close);
            }
         }
         var node:Object = table;
         for(var s:int = 0; s + 1 < steps.length; s++)
         {
            if(node[steps[s]] == null)
            {
               node[steps[s]] = steps[s + 1] is int ? [] : {};
            }
            node = node[steps[s]];
         }
         var number:Number = Number(text);
         node[steps[steps.length - 1]] = text != "" && !isNaN(number) ? number : text;
      }
   }
}
