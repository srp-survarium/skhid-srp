package survarium.item
{
   import flash.display.Sprite;
   import survarium.controls.LabelDiffIndicator;
   import survarium.controls.ItemCompare;
   import survarium.player.MatchEffects;
   import temp_test_fla.weapon_compare_mc_8;

   // Use the exported native symbol, not a new TextField: Scaleform resolves
   // the game's embedded font on its timeline fields, unlike Ruffle's fallback.
   public final class WeaponDetailsView extends Sprite
   {
      private var contentHeight:Number = 0;
      override public function get height() : Number { return contentHeight; }
      public function WeaponDetailsView()
      {
         mouseEnabled = false;
         mouseChildren = false;
      }

      // effects: the kit's and the skills' while Alt is held (details already with them), null for
      // the weapon's own stats.
      public function show(details:WeaponDetails, expanded:Boolean, original:ItemCompare, effects:MatchEffects = null) : void
      {
         while(numChildren) removeChildAt(numChildren - 1);
         contentHeight = 0;
         if(details == null) return;
         x = original.x;
         scaleX = original.scaleX;
         scaleY = original.scaleY;
         var stride:Number = original.diff_2.y - original.diff_1.y;
         var nextY:Number = 0;
         var secondary:WeaponDetails = WeaponDetails.read(original.getSecondaryData(),effects);
         var entries:Array = details.rows(expanded,secondary);
         // Under the heading, one line saying what is on ("With kit and skills").
         if(effects != null) entries.splice(1,0,{kind:"effects",text:effects.label});
         var peer:Array = secondary == null ? [] : secondary.rows(expanded,details);
         var peerStats:Object = {};
         for each(var other:Object in peer) if(other.kind == "stat") peerStats[other.id] = other;
         for(var index:int = 0; index < entries.length; index++)
         {
            var entry:Object = entries[index];
            if(entry.kind != "stat")
            {
               var caption:WeaponDetailCaption = new WeaponDetailCaption(entry);
               addChild(caption);
               if(entry.kind == "heading" && index > 0) nextY += 6;
               caption.x += original.diff_1.x;
               caption.y += original.diff_1.y + nextY;
               nextY += caption.contentHeight;
               continue;
            }
            var wrapper:weapon_compare_mc_8 = new weapon_compare_mc_8();
            addChild(wrapper);
            wrapper.x = original.diff_1.x;
            wrapper.y = original.diff_1.y + nextY;
            wrapper.background.visible = false;
            var row:LabelDiffIndicator = wrapper.indicator;
            var compared:Object = peerStats[entry.id];
            if(entry.comparison.kind == "lower-range")
            {
               // A range is better only if neither endpoint is worse.
               // Crossing ranges stay neutral instead of comparing averages.
               row.direction = false;
               row.value = row.compare = 0;
               if(compared != null && compared.comparison.kind == "lower-range")
               {
                  var low:Number = entry.comparison.low - compared.comparison.low;
                  var high:Number = entry.comparison.high - compared.comparison.high;
                  // Native indicators clamp numbers below zero.
                  if(low <= 0 && high <= 0 && (low < 0 || high < 0)) row.compare = 1;
                  else if(low >= 0 && high >= 0 && (low > 0 || high > 0)) row.value = 1;
               }
            }
            else if(entry.comparison.kind != "neutral")
            {
               row.direction = entry.comparison.kind == "higher";
               row.value = entry.comparison.value;
               // A dash (no such stat) compares with nothing.
               row.compare = compared == null || compared.comparison.kind != entry.comparison.kind ? entry.comparison.value : compared.comparison.value;
            }
            else row.value = row.compare = 0;
            row.setDetailText(entry.label,entry.value,entry.verdict);
            row.validateNow();
            var lines:int = row.detailLines;
            // Both cards reserve the larger label height for a compared row.
            // Different falloff distances can wrap differently in translation.
            // Held Alt, a wider value ("base -> in a match") leaves its label less room, too.
            if(compared != null && (compared.label != entry.label || effects != null && compared.value != entry.value))
               lines = Math.max(lines,linesOf(compared));
            nextY += stride * Math.max(1,lines);
         }
         contentHeight = nextY * scaleY;
      }

      // The lines a row's label takes beside its value, laid out on a row of its own.
      private function linesOf(entry:Object) : int
      {
         var probe:weapon_compare_mc_8 = new weapon_compare_mc_8();
         addChild(probe);
         probe.indicator.setDetailText(entry.label,entry.value);
         probe.indicator.validateNow();
         var lines:int = probe.indicator.detailLines;
         removeChild(probe);
         return lines;
      }
   }
}
