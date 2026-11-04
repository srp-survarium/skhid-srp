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
   public class ModeNotice extends Sprite
   {
      public static const DISPLAY_MS:int = 10000;
      private var _state:Object = {kind:"inactive"};
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

      public function reset() : void
      {
         _state = {kind:"inactive"};
         label.text = "";
         visible = false;
      }

      public function update(now:int, ready:Boolean, countdown:Boolean) : void
      {
         if(_state.kind == "waiting" && ready && ModeIntroText.ready(_state.mode))
         {
            label.text = ModeIntroText.start(_state.mode);
            _state = countdown ? {kind:"countdown"} : {kind:"showing",until:now + DISPLAY_MS};
            visible = true;
         }
         if(_state.kind == "countdown" && !countdown)
            _state = {kind:"showing",until:now + DISPLAY_MS};
         if(_state.kind == "showing" && int(now - _state.until) >= 0)
         {
            _state = {kind:"finished"};
            visible = false;
         }
      }

      public function place(screenWidth:Number, timerBottom:Number) : void
      {
         label.width = Math.min(600,Math.max(200,screenWidth - 40));
         x = (screenWidth - label.width) / 2;
         y = Math.max(65,timerBottom + 12);
      }
   }
}
