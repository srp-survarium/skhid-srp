package survarium.modes
{
   import flash.display.Sprite;
   import flash.filters.DropShadowFilter;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import scaleform.clik.layout.LayoutData;

   // HUD-local lifetime: joining an old match still gets a full reading interval.
   // Native countdown updates cannot clear this separate label.
   // Shown only when the match's word says so (root.mode_notice, exe patch 214, told after the hud's
   // reset at each match's start: the account's first three matches of the mode,
   // docs/notes/mode_introductions.md); a notice begun waits for it.
   public class ModeNotice extends Sprite
   {
      // On screen after the countdown: Freeze Tag's longer (the owner, 2026-11-26: it went too fast).
      public static function displayMs(mode:String) : int
      {
         return mode == "freeze_tag" ? 20000 : 10000;
      }
      private var _state:Object = {kind:"inactive"};
      // This match's word: "unknown" until told, then "shown" or "skipped" until the next reset.
      private var _word:String = "unknown";
      public var label:TextField = new TextField();
      public var layoutData:LayoutData;

      // The whole format of `source`, larger: its font is embedded in one cut only (Open Sans
      // Condensed Bold), and a format naming the font without its bold finds none (boxes).
      public function ModeNotice(source:TextField)
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

      public function begin(mode:String) : void
      {
         if(_state.kind == "inactive")
            _state = {kind:"waiting",mode:mode};
      }

      // A new match: no notice, and no word yet.
      public function reset() : void
      {
         _word = "unknown";
         stop();
      }

      // The match's word, once (a late join's rebuild tells it again: the same).
      public function tell(shown:Boolean) : void
      {
         if(_word == "unknown")
            _word = shown ? "shown" : "skipped";
      }

      private function stop() : void
      {
         _state = {kind:"inactive"};
         label.text = "";
         visible = false;
      }

      // Reset only while it is `mode`'s notice: another mode's goes on (Blackout unforced at a Freeze
      // Tag match's start, another game type after Freeze Tag's).
      public function cancel(mode:String) : void
      {
         if(_state.mode == mode)
            stop();
      }

      public function update(now:int, ready:Boolean, countdown:Boolean) : void
      {
         if(_state.kind == "waiting" && ready && _word == "shown" && ModeIntroText.ready(_state.mode))
         {
            label.text = ModeIntroText.start(_state.mode);
            _state = countdown ? {kind:"countdown",mode:_state.mode} : {kind:"showing",mode:_state.mode,until:now + displayMs(_state.mode)};
            visible = true;
         }
         if(_state.kind == "countdown" && !countdown)
            _state = {kind:"showing",mode:_state.mode,until:now + displayMs(_state.mode)};
         if(_state.kind == "showing" && int(now - _state.until) >= 0)
         {
            _state = {kind:"finished",mode:_state.mode};
            visible = false;
         }
      }

      // Counting down or showing its text: the frames the HUD places it and runs its end (update).
      public function get shown() : Boolean
      {
         return _state.kind == "countdown" || _state.kind == "showing";
      }

      public function place(screenWidth:Number, timerBottom:Number) : void
      {
         label.width = Math.min(600,Math.max(200,screenWidth - 40));
         x = (screenWidth - label.width) / 2;
         y = Math.max(65,timerBottom + 12);
      }
   }
}
