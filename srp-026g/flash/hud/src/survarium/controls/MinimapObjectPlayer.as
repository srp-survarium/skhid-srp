package survarium.controls
{
   import flash.filters.GlowFilter;
   import flash.geom.Rectangle;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;

   public class MinimapObjectPlayer extends MinimapObject
   {

      // A name's text: the Tab list's name font (its rows' FixedTextPlayerName), smaller, white
      // with a dark edge; read once.
      public static const NAME_SIZE:int = 13;

      private static var _nameFormat:TextFormat;

      private static var _nameEmbedded:Boolean;

      protected var _type:int = -1;

      // The name over the icon (showName), made at its first name and kept with the icon.
      private var _label:TextField;

      public function MinimapObjectPlayer()
      {
         super();
         stop();
      }

      public function set type(value:int) : void
      {
         gotoAndStop(value);
         _type = value;
      }

      // The player's name centred over the icon; null shows none. Its text and place change only
      // when the name does.
      public function showName(name:String) : void
      {
         if(name == null)
         {
            if(this._label)
            {
               this._label.visible = false;
            }
            return;
         }
         if(!this._label)
         {
            _label = makeLabel();
         }
         if(this._label.text != name || this._label.parent != this)
         {
            if(this._label.parent == this)
            {
               removeChild(this._label);
            }
            var icon:Rectangle = getBounds(this);
            this._label.text = name;
            this._label.x = -this._label.width / 2;
            this._label.y = icon.top - this._label.height + 2;
            addChild(this._label);
         }
         this._label.visible = true;
      }

      // The name shown over the icon, or null.
      public function get shownName() : String
      {
         return this._label && this._label.visible ? this._label.text : null;
      }

      private static function makeLabel() : TextField
      {
         var label:TextField = new TextField();
         if(!_nameFormat)
         {
            var source:TextField = new FixedTextPlayerName().textField;
            _nameFormat = source.length > 0 ? source.getTextFormat() : source.defaultTextFormat;
            _nameFormat.size = NAME_SIZE;
            _nameFormat.color = 0xFFFFFF;
            _nameFormat.align = "left";
            _nameEmbedded = source.embedFonts;
         }
         label.defaultTextFormat = _nameFormat;
         label.embedFonts = _nameEmbedded;
         label.selectable = false;
         label.mouseEnabled = false;
         label.multiline = false;
         label.wordWrap = false;
         label.autoSize = TextFieldAutoSize.LEFT;
         label.filters = [new GlowFilter(0,0.9,3,3,4)];
         return label;
      }
   }
}
