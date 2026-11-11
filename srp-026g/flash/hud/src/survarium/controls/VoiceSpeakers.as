package survarium.controls
{
   import com.greensock.TweenLite;
   import flash.display.Graphics;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import scaleform.clik.core.UIComponent;

   // Voice chat: who is talking (exe patch 66 calls HUD.srp_voice_talking with the names on every
   // change; docs/notes/voice-chat.md): the teammates heard, and the player's own name while its
   // microphone is sent. As 0.69's VoiceChatTalkingPanel shows it: a row each, a light-blue
   // speaker and the name, 25 px apart, the newest talker on top; a name stays while its player
   // talks and fades out over 1 s, 0.5 s after the player stops. Muted players never show (the exe
   // leaves them out). The rows end at the component's x (the hud puts it at the right edge): the
   // name, the speaker left of it. Not clickable.
   public class VoiceSpeakers extends UIComponent
   {

      public static const ROW_HEIGHT:Number = 25;

      // The most rows; the exe sends at most a team.
      public static const MAX_ROWS:uint = 10;

      // 0.69's fade: after FADE_DELAY s, over FADE_S s.
      public static const FADE_DELAY:Number = 0.5;

      public static const FADE_S:Number = 1;

      protected static const TEXT_SIZE:Number = 16;

      // The speaker's drawing: 24 px wide, its middle at this y.
      protected static const SPEAKER_WIDTH:Number = 24;

      protected static const SPEAKER_MIDDLE:Number = 13;

      // 0.69's speaker (HudVoicePanelTalkingPlayer's)
      protected static const SPEAKER:uint = 0x73B9FF;

      protected static const NAME:uint = 0xFFFFFF;

      // The rows, top first: {sprite, name, talking}.
      protected var _rows:Array = [];

      // DefaultLabel3's embedded font, as the Tab list's player menu takes it: the label supplies
      // the font only (a Label on the stage draws its text lower in the game than its field is
      // high, and cut).
      protected var _format:TextFormat;

      protected var _embed:Boolean;

      public function VoiceSpeakers()
      {
         super();
         mouseEnabled = false;
         mouseChildren = false;
         var source:TextField = new DefaultLabel3().textField;
         _embed = source.embedFonts;
         _format = source.length > 0 ? source.getTextFormat() : source.defaultTextFormat;
         this._format.size = TEXT_SIZE;
         this._format.color = NAME;
         this._format.align = "left";
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
      }

      protected function makeRow(name:String) : Object
      {
         var sprite:Sprite = new Sprite();
         var field:TextField = new TextField();
         field.defaultTextFormat = this._format;
         field.embedFonts = this._embed;
         field.selectable = false;
         field.mouseEnabled = false;
         field.multiline = false;
         field.wordWrap = false;
         field.autoSize = TextFieldAutoSize.LEFT;
         field.text = name;
         // the name ends at the row's x, its middle at the speaker's; the speaker left of it
         field.x = -field.width;
         field.y = SPEAKER_MIDDLE - field.height / 2;
         sprite.addChild(field);
         var mark:Shape = new Shape();
         drawSpeaker(mark.graphics);
         mark.x = field.x - SPEAKER_WIDTH - 2;
         sprite.addChild(mark);
         addChild(sprite);
         return {sprite:sprite, name:name, talking:false};
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
