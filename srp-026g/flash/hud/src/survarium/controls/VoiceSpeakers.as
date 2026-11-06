package survarium.controls
{
   import com.greensock.TweenLite;
   import flash.display.Graphics;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.TimerEvent;
   import flash.geom.ColorTransform;
   import flash.text.TextFieldAutoSize;
   import flash.utils.Timer;
   import scaleform.clik.core.UIComponent;

   // Voice chat: which teammates are talking (exe patch 66 calls HUD.srp_voice_talking with their
   // names on every change; docs/notes/voice-chat.md), as 0.69's VoiceChatTalkingPanel shows it:
   // a row each, a light-blue speaker and the name, 25 px apart, the newest talker on top; a
   // name stays while its player talks and fades out over 1 s, 0.5 s after the player stops.
   // Muted players never show (the exe leaves them out), nor the player's own. Not clickable.
   public class VoiceSpeakers extends UIComponent
   {

      public static const ROW_HEIGHT:Number = 25;

      // The most rows; the exe sends at most a team.
      public static const MAX_ROWS:uint = 10;

      // 0.69's fade: after FADE_DELAY s, over FADE_S s.
      public static const FADE_DELAY:Number = 0.5;

      public static const FADE_S:Number = 1;

      protected static const TEXT_SCALE:Number = 0.6;

      // 0.69's speaker (HudVoicePanelTalkingPlayer's)
      protected static const SPEAKER:uint = 0x73B9FF;

      protected static const NAME:uint = 0xFFFFFF;

      // The rows, top first: {sprite, name, talking}.
      protected var _rows:Array = [];

      // A short line under the rows ("Voice muted: name"), for NOTICE_MS.
      public static const NOTICE_MS:uint = 3000;

      protected var _notice:DefaultLabel3;

      protected var _noticeTimer:Timer;

      public function VoiceSpeakers()
      {
         super();
         mouseEnabled = false;
         mouseChildren = false;
      }

      // names: the teammates talking now.
      public function update(names:Array) : void
      {
         var talking:Array = names.slice(0,MAX_ROWS);
         var now:Object = {};
         var name:String;
         for each(name in talking)
         {
            now[name] = true;
         }
         var row:Object;
         for each(row in this._rows)
         {
            if(row.talking && !now[row.name])
            {
               row.talking = false;
               this.fade(row);
            }
         }
         for each(name in talking)
         {
            row = this.rowOf(name);
            if(row != null && row.talking)
            {
               continue;
            }
            if(row != null)
            {
               this._rows.splice(this._rows.indexOf(row),1);
            }
            else
            {
               row = this.makeRow(name);
            }
            row.talking = true;
            TweenLite.killTweensOf(row.sprite);
            row.sprite.alpha = 1;
            this._rows.unshift(row);
         }
         this.place();
      }

      public function clear() : void
      {
         for each(var row:Object in this._rows)
         {
            TweenLite.killTweensOf(row.sprite);
            removeChild(row.sprite);
         }
         this._rows = [];
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
            this._notice.transform.colorTransform = tint(SPEAKER);
            addChild(this._notice);
            _noticeTimer = new Timer(NOTICE_MS,1);
            this._noticeTimer.addEventListener(TimerEvent.TIMER_COMPLETE,this.onNoticeOver);
         }
         this._notice.text = text;
         this._notice.visible = true;
         this.place();
         this._noticeTimer.reset();
         this._noticeTimer.start();
      }

      protected function onNoticeOver(event:TimerEvent) : void
      {
         this._notice.visible = false;
      }

      protected function rowOf(name:String) : Object
      {
         for each(var row:Object in this._rows)
         {
            if(row.name == name)
            {
               return row;
            }
         }
         return null;
      }

      protected function fade(row:Object) : void
      {
         TweenLite.killTweensOf(row.sprite);
         TweenLite.to(row.sprite,FADE_S,{
            "delay":FADE_DELAY,
            "alpha":0,
            "onComplete":this.drop,
            "onCompleteParams":[row]
         });
      }

      // A faded row goes (unless its player talks again meanwhile).
      protected function drop(row:Object) : void
      {
         var at:int = this._rows.indexOf(row);
         if(at >= 0 && !row.talking)
         {
            this._rows.splice(at,1);
            removeChild(row.sprite);
            this.place();
         }
      }

      protected function place() : void
      {
         for(var i:int = 0; i < this._rows.length; i++)
         {
            this._rows[i].sprite.y = i * ROW_HEIGHT;
         }
         if(this._notice)
         {
            this._notice.y = this._rows.length * ROW_HEIGHT;
         }
      }

      protected function makeRow(name:String) : Object
      {
         var sprite:Sprite = new Sprite();
         var mark:Shape = new Shape();
         drawSpeaker(mark.graphics);
         sprite.addChild(mark);
         var label:DefaultLabel3 = new DefaultLabel3();
         label.textField.wordWrap = false;
         label.textField.multiline = false;
         label.autoSize = TextFieldAutoSize.LEFT;
         label.scaleX = label.scaleY = TEXT_SCALE;
         label.x = 26;
         label.y = -3;
         label.text = name;
         // Tinted, not recoloured: the label's frame script and its redraws reset the text's own
         // format to the symbol's (white).
         label.transform.colorTransform = tint(NAME);
         sprite.addChild(label);
         addChild(sprite);
         return {sprite:sprite, name:name, talking:false};
      }

      protected static function tint(colour:uint) : ColorTransform
      {
         return new ColorTransform(((colour >> 16) & 0xFF) / 255,((colour >> 8) & 0xFF) / 255,(colour & 0xFF) / 255);
      }

      // 0.69's speaker: a box, a cone, two waves (lines, no circles: a circle after lineStyle
      // draws a stray line from the pen).
      protected static function drawSpeaker(g:Graphics) : void
      {
         g.lineStyle();
         g.beginFill(SPEAKER,1);
         g.drawRect(2,9,5,8);
         g.endFill();
         g.beginFill(SPEAKER,1);
         g.moveTo(7,9);
         g.lineTo(13,4);
         g.lineTo(13,22);
         g.lineTo(7,17);
         g.lineTo(7,9);
         g.endFill();
         g.lineStyle(2,SPEAKER,1);
         g.moveTo(16,9);
         g.curveTo(18,13,16,17);
         g.moveTo(19,6);
         g.curveTo(22.5,13,19,20);
      }
   }
}
