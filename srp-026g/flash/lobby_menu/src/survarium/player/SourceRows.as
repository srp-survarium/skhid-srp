package survarium.player
{
   import survarium.Util;

   // The tooltip rows of named sources ({name, value}): one row each but 0,
   // "<name> <value>", biggest increase first and biggest decrease last, green
   // an increase and red a decrease (the rows' own style sheet).
   public final class SourceRows
   {
      public static function rows(sources:Array, icon:uint, text:Function) : Array
      {
         var shown:Array = [];
         for each(var source:Object in sources)
            if(Math.round(source.value * 1000) != 0) shown.push(source);
         shown.sortOn("value",Array.NUMERIC | Array.DESCENDING);
         var result:Array = [];
         for each(var row:Object in shown)
            result.push({icon:icon,label:row.name + " " + Util.styledChar(text(row.value),row.value > 0 ? "green" : "red")});
         return result;
      }
   }
}
