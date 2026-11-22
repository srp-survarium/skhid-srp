package survarium.modes
{
   import flash.display.Sprite;
   import flash.geom.Rectangle;
   import flash.text.TextField;

   // Artifact Hunt's respawn wave (docs/notes/artefact_hunt.md "HUD calls", #21, #22): every team's
   // next wave, a smaller number of seconds right after the match time, on its baseline, for the
   // living and the dead while one comes. A dead player's own time stays the game's respawn line.
   public class ArtefactHuntWave extends Sprite
   {
      protected static const SIZE:Number = 20;

      protected static const GAP:Number = 3;

      protected var _number:TextField;

      // The room after the match time: its widest number and the gap.
      protected var _room:Number;

      public function ArtefactHuntWave(font:TextField)
      {
         super();
         this._number = ArtefactHuntLook.text(font,SIZE);
         this._number.visible = true;
         this._number.text = "00";
         this._room = Math.ceil(this._number.width) + GAP;
         addChild(this._number);
         visible = false;
      }

      // The room it takes after the match time, in this sprite's units.
      public function get room() : Number
      {
         return this._room;
      }

      // At `now` after `clock`: the match time's text in the row's units, its bottom the digits'
      // baseline (null: not shown).
      public function draw(s:ArtefactHuntState, now:int, clock:Rectangle) : void
      {
         var left:Number = s == null ? 0 : s.waveLeftAt(now);
         visible = left > 0 && clock != null;
         if(!visible)
         {
            return;
         }
         var n:TextField = this._number;
         var text:String = String(Math.ceil(left / 1000));
         if(n.text != text)
         {
            n.text = text;
         }
         n.x = Math.round(clock.right + GAP);
         n.y = Math.round(clock.bottom - 2 - n.getLineMetrics(0).ascent);
      }

      // The number shown, "" none (for checks).
      public function get text() : String
      {
         return visible ? this._number.text : "";
      }
   }
}
