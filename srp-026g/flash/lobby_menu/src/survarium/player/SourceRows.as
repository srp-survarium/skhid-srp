package survarium.player
{
   import survarium.Util;
   import survarium.controls.FiltersWindow;

   // The tooltip rows of named sources ({name, value, category}): one row each but 0,
   // "<name> <value>", biggest increase first and biggest decrease last, green
   // an increase and red a decrease (the rows' own style sheet). An item's row has
   // its group's icon, the shop filter's (FiltersWindow.filterIcon: `group`); a
   // source without one (the weight, a skill) the modifier icon `icon`.
   public final class SourceRows
   {
      // `lowerIsBetter`: a decrease is green (stamina use); else an increase is.
      public static function rows(sources:Array, icon:uint, text:Function, lowerIsBetter:Boolean = false) : Array
      {
         var shown:Array = [];
         for each(var source:Object in sources)
            if(Math.round(source.value * 1000) != 0) shown.push(source);
         shown.sortOn("value",lowerIsBetter ? Array.NUMERIC : Array.NUMERIC | Array.DESCENDING);
         var result:Array = [];
         for each(var row:Object in shown)
         {
            var entry:Object = {icon:icon,label:row.name + " " + Util.styledChar(text(row.value),(row.value > 0) != lowerIsBetter ? "green" : "red")};
            var group:int = row.category != undefined ? FiltersWindow.filterIcon(row.category) : -1;
            if(group >= 0) entry.group = group;
            result.push(entry);
         }
         return result;
      }
   }
}
