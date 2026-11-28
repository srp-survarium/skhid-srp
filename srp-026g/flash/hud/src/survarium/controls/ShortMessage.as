package survarium.controls
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;

   // The game's short message (a SrvContextMessage3: set_parameterized_message's, and Freeze Tag's
   // reveal popup, a copy of its own), timed by the hud's clock (getTimer) from the time it was told,
   // not by its timeline. The owner, 2026-11-27: "freeze messages like you are revealed disappear
   // almost immediately", "the same is true for FFA end message (victory or loss)". Both are a
   // match's second message or later in the clip: the first ran its timeline to its end, whose
   // frame-30 script takes the clip off the display list, and the next one was started again by
   // gotoAndPlay(0), a frame that does not exist (frames count from 1). The game's Scaleform 4.2
   // makes a frame number n the frame index n-1 (MovieClip.gotoAndPlay, 0x760120), so 0 gives
   // 0xFFFFFFFF, never the current frame: a goto every time, to wherever its sprite's GotoFrame takes
   // that (not read). Ruffle takes it for frame 1 and shows the message its whole time, so the cut
   // was the game's only. Here nothing depends on it: the clip stays on the display list, held at
   // the frame that holds a message (where its timeline stops), its fades its alpha; none of its
   // timeline's frame scripts runs again and no timeout ends it.
   // A message told again while shown (the same text, or another) is refreshed: its text, a full
   // hold again, its alpha going on from where it is (no fade from nothing, no flicker).
   public class ShortMessage
   {
      // The stock fades: the clip's 15 frames in and 15 out at the movie's 30 fps.
      public static const FADE_MS:int = 500;
      // The clip's frame where its timeline stops to hold a message.
      private static const HOLD_FRAME:int = 15;

      private var _clip:MovieClip;
      private var _field:TextField;
      private var _format:TextFormat;
      // null: hidden.
      private var _shown:Shown = null;

      // Its own text's format, taken while it has its authored text: an empty field's names no
      // embedded font.
      public function ShortMessage(clip:MovieClip)
      {
         _clip = clip;
         _clip.gotoAndStop(HOLD_FRAME);
         _clip.mouseEnabled = _clip.mouseChildren = false;
         _clip.visible = false;
         _field = _clip.mc_text.contextText;
         _format = _field.length > 0 ? _field.getTextFormat() : _field.defaultTextFormat;
         _field.autoSize = TextFieldAutoSize.LEFT;
      }

      public function get clip() : MovieClip
      {
         return _clip;
      }

      // Its text while shown, else null.
      public function get text() : String
      {
         return _shown != null ? _shown.text : null;
      }

      // Shows `text` in its `size` from `now`, fully for `holdMs` (Infinity: until hidden or told
      // again), or refreshes the message shown.
      public function show(text:String, size:uint, holdMs:Number, now:int) : void
      {
         var changed:Boolean = _shown == null || _shown.text != text || _format.size != size;
         _shown = new Shown(text,now - alphaAt(now) * FADE_MS,now + holdMs);
         if(changed)
         {
            _format.size = size;
            _field.defaultTextFormat = _format;
            _field.text = text;
            _field.setTextFormat(_format);
         }
         _clip.visible = true;
         draw(now);
      }

      // Its fade-out from `now`, unless it is fading already.
      public function fadeOut(now:int) : void
      {
         if(_shown != null && _shown.until > now)
            _shown = new Shown(_shown.text,_shown.start,now);
      }

      // Its alpha at `now`; false once it is over (then hidden).
      public function draw(now:int) : Boolean
      {
         if(_shown == null)
            return false;
         if(now - _shown.until >= FADE_MS)
         {
            hide();
            return false;
         }
         _clip.alpha = alphaAt(now);
         return true;
      }

      public function hide() : void
      {
         _shown = null;
         _clip.visible = false;
      }

      // 0 to 1: fading in from its start, held, fading out after its hold.
      private function alphaAt(now:int) : Number
      {
         if(_shown == null)
            return 0;
         var fadeIn:Number = (now - _shown.start) / FADE_MS;
         var fadeOut:Number = 1 - (now - _shown.until) / FADE_MS;
         return Math.max(0,Math.min(1,fadeIn,fadeOut));
      }
   }
}

// A message shown: its text, when its fade-in began, until when it is held (the hud's clock, ms).
class Shown
{
   public var text:String;
   public var start:Number;
   public var until:Number;

   public function Shown(text:String, start:Number, until:Number)
   {
      this.text = text;
      this.start = start;
      this.until = until;
   }
}
