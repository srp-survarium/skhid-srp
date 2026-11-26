package survarium.modes
{
   import flash.display.Sprite;
   import flash.filters.DropShadowFilter;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import survarium.data.LocalizationData;

   // Freeze Tag: the countdown to a last one fighting's reveal, for everyone (the owner, 2026-11-26:
   // "the last person should only be seen after 15 seconds, with a clear timer visible for
   // everyone"): under the match timer, in the mode notice's style (ModeNotice), a line for each
   // side down to its last one, as the exe tells it every frame (HUD.freeze_tag_reveal, from the
   // rule's state and the match's time, the same in every game): its seconds left, then for 3 s that
   // he is revealed, then nothing.
   public class RevealNotice extends Sprite
   {
      // Whose (the exe's numbers): the viewer himself, his last teammate, the enemy's last one.
      public static const VIEWER:uint = 1;
      public static const TEAMMATE:uint = 2;
      public static const ENEMY:uint = 3;

      public var label:TextField = new TextField();

      private var _told:String = "";

      // The whole format of `source` (the countdown's), larger, as ModeNotice's.
      public function RevealNotice(source:TextField)
      {
         mouseEnabled = mouseChildren = false;
         label.mouseEnabled = label.selectable = false;
         var format:TextFormat = source.length > 0 ? source.getTextFormat() : source.defaultTextFormat;
         format.size = 18;
         format.color = 0xFFFFFF;
         format.align = "center";
         label.defaultTextFormat = format;
         label.embedFonts = source.embedFonts;
         label.autoSize = TextFieldAutoSize.LEFT;
         label.multiline = label.wordWrap = true;
         label.width = 600;
         label.filters = [new DropShadowFilter(1,90,0,1,3,3,2)];
         addChild(label);
         visible = false;
      }

      // Each side's: whose (0 none) and the ms left until he is revealed (0: he is).
      public function tell(own:uint, ownLeft:uint, enemy:uint, enemyLeft:uint) : void
      {
         var lines:Array = [];
         for each(var side:Array in [[own,ownLeft],[enemy,enemyLeft]])
         {
            var text:String = line(side[0],side[1]);
            if(text != null)
            {
               lines.push(text);
            }
         }
         var told:String = lines.join("\n");
         if(told != this._told)
         {
            _told = told;
            label.text = told;
         }
         visible = told != "";
      }

      public function reset() : void
      {
         this.tell(0,0,0,0);
      }

      public function get shown() : Boolean
      {
         return visible;
      }

      // A side's line: its seconds left, rounded up, or that he is revealed; null for none.
      private static function line(who:uint, left:uint) : String
      {
         if(who != VIEWER && who != TEAMMATE && who != ENEMY)
         {
            return null;
         }
         if(left > 0)
         {
            return LocalizationData.format(who == VIEWER ? "st_srp_hud_freeze_reveal_self" :
               who == TEAMMATE ? "st_srp_hud_freeze_reveal_teammate" : "st_srp_hud_freeze_reveal_enemy",
               String(Math.ceil(left / 1000)));
         }
         return LocalizationData.getName(who == VIEWER ? "st_srp_hud_freeze_revealed_self" :
            who == TEAMMATE ? "st_srp_hud_freeze_revealed_teammate" : "st_srp_hud_freeze_revealed_enemy");
      }

      // Centred, under `bottom` (the timer's, or the mode notice's while it shows).
      public function place(screenWidth:Number, bottom:Number) : void
      {
         label.width = Math.min(600,Math.max(200,screenWidth - 40));
         x = (screenWidth - label.width) / 2;
         y = Math.max(65,bottom + 12);
      }
   }
}
