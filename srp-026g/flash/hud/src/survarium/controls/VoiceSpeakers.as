package survarium.controls
{
   import flash.display.Graphics;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.TimerEvent;
   import flash.geom.ColorTransform;
   import flash.text.TextFieldAutoSize;
   import flash.utils.Timer;
   import scaleform.clik.core.UIComponent;

   // Voice chat: who of the team is talking now (exe patch 66 calls HUD.srp_voice_talking on
   // every change; docs/notes/voice-chat.md). A row each: a speaker mark and the name. The
   // player's own row while it transmits is gold; a teammate heard through the radio (farther
   // than 25 m) has the mark's waves dimmed. Muted players never show (the exe leaves them out).
   // Nothing when nobody talks. Not clickable.
   public class VoiceSpeakers extends UIComponent
   {

      public static const ROW_HEIGHT:Number = 26;

      // The most rows shown; the exe sends at most a team.
      public static const MAX_ROWS:uint = 10;

      // srp_voice_talking's flags, per name.
      public static const FLAG_SELF:uint = 1;

      public static const FLAG_RADIO:uint = 2;

      protected static const TEXT_SCALE:Number = 0.6;

      protected static const OWN:uint = 0xE8C268;

      protected static const TEAM:uint = 0xFFFFFF;

      protected var _rows:Array = [];

      // The names and flags shown, to redraw on a change only.
      protected var _shown:String = "";

      // A short line under the rows ("Muted: name"), for NOTICE_MS.
      public static const NOTICE_MS:uint = 3000;

      protected var _notice:DefaultLabel3;

      protected var _noticeTimer:Timer;

      public function VoiceSpeakers()
      {
         super();
         mouseEnabled = false;
         mouseChildren = false;
      }

      // pairs: name, flags (FLAG_SELF, FLAG_RADIO), in the order to show.
      public function update(pairs:Array) : void
      {
         var key:String = pairs.join("\n");
         if(key == this._shown)
         {
            return;
         }
         _shown = key;
         var count:uint = Math.min(uint(pairs.length / 2),MAX_ROWS);
         while(this._rows.length < count)
         {
            this._rows.push(this.makeRow());
         }
         var i:uint = 0;
         var row:Sprite;
         while(i < this._rows.length)
         {
            row = this._rows[i];
            if(i < count)
            {
               this.fillRow(row,String(pairs[2 * i]),uint(pairs[2 * i + 1]));
               row.y = i * ROW_HEIGHT;
               if(row.parent != this)
               {
                  addChild(row);
               }
            }
            else if(row.parent == this)
            {
               removeChild(row);
            }
            i++;
         }
         if(this._notice)
         {
            this._notice.y = count * ROW_HEIGHT;
         }
      }

      public function clear() : void
      {
         this.update([]);
         if(this._notice)
         {
            this._notice.visible = false;
         }
      }

      // The mute key's answer (HUD.srp_voice_notice): the line under the rows, then gone.
      public function notice(text:String) : void
      {
         if(!this._notice)
         {
            _notice = new DefaultLabel3();
            this._notice.textField.wordWrap = false;
            this._notice.textField.multiline = false;
            this._notice.autoSize = TextFieldAutoSize.LEFT;
            this._notice.scaleX = this._notice.scaleY = TEXT_SCALE;
            this._notice.transform.colorTransform = new ColorTransform(((OWN >> 16) & 0xFF) / 255,((OWN >> 8) & 0xFF) / 255,(OWN & 0xFF) / 255);
            addChild(this._notice);
            _noticeTimer = new Timer(NOTICE_MS,1);
            this._noticeTimer.addEventListener(TimerEvent.TIMER_COMPLETE,this.onNoticeOver);
         }
         this._notice.text = text;
         this._notice.y = this.rowsShown() * ROW_HEIGHT;
         this._notice.visible = true;
         this._noticeTimer.reset();
         this._noticeTimer.start();
      }

      protected function onNoticeOver(event:TimerEvent) : void
      {
         this._notice.visible = false;
      }

      protected function rowsShown() : uint
      {
         var n:uint = 0;
         for each(var row:Sprite in this._rows)
         {
            if(row.parent == this)
            {
               n++;
            }
         }
         return n;
      }

      protected function makeRow() : Sprite
      {
         var row:Sprite = new Sprite();
         var mark:Shape = new Shape();
         mark.name = "mark";
         row.addChild(mark);
         var label:DefaultLabel3 = new DefaultLabel3();
         label.name = "label";
         label.textField.wordWrap = false;
         label.textField.multiline = false;
         label.autoSize = TextFieldAutoSize.LEFT;
         label.scaleX = label.scaleY = TEXT_SCALE;
         label.x = 26;
         label.y = -3;
         row.addChild(label);
         return row;
      }

      protected function fillRow(row:Sprite, name:String, flags:uint) : void
      {
         var colour:uint = (flags & FLAG_SELF) != 0 ? OWN : TEAM;
         var label:DefaultLabel3 = row.getChildByName("label") as DefaultLabel3;
         label.text = name;
         // Tinted, not recoloured: the label's frame script and its redraws reset the text's own
         // format to the symbol's (white).
         label.transform.colorTransform = new ColorTransform(((colour >> 16) & 0xFF) / 255,((colour >> 8) & 0xFF) / 255,(colour & 0xFF) / 255);
         drawMark(Shape(row.getChildByName("mark")).graphics,colour,(flags & FLAG_RADIO) != 0);
      }

      // A loudspeaker: a box, a cone, two waves (lines, no circles: a circle after lineStyle
      // draws a stray line from the pen).
      protected static function drawMark(g:Graphics, colour:uint, radio:Boolean) : void
      {
         g.clear();
         g.lineStyle();
         g.beginFill(colour,1);
         g.drawRect(2,9,5,8);
         g.endFill();
         g.beginFill(colour,1);
         g.moveTo(7,9);
         g.lineTo(13,4);
         g.lineTo(13,22);
         g.lineTo(7,17);
         g.lineTo(7,9);
         g.endFill();
         g.lineStyle(2,colour,radio ? 0.35 : 1);
         g.moveTo(16,9);
         g.curveTo(18,13,16,17);
         g.moveTo(19,6);
         g.curveTo(22.5,13,19,20);
      }
   }
}
