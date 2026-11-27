package survarium.controls
{
   import flash.display.DisplayObject;
   import flash.filters.GlowFilter;
   import flash.geom.ColorTransform;
   import flash.geom.Rectangle;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import survarium.modes.FrozenPlaces;

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

      // Freeze Tag: the glyph's own colour while the icon is drawn frozen (null: not frozen).
      private var _ownGlyph:ColorTransform = null;

      public function MinimapObjectPlayer()
      {
         super();
         stop();
      }

      // A frame of another class has another glyph: a frozen icon stays frozen.
      public function set type(value:int) : void
      {
         var frozen:Boolean = this.frozen;
         this.frozen = false;
         gotoAndStop(value);
         _type = value;
         this.frozen = frozen;
      }

      // Freeze Tag (Minimap: a frozen teammate): the glyph and the name in the ice blue, as the squad's
      // glyph is green; the own colours back on thaw.
      public function set frozen(value:Boolean) : void
      {
         var glyph:DisplayObject = this.glyph;
         if(value == this.frozen || glyph == null)
         {
            return;
         }
         if(value)
         {
            _ownGlyph = glyph.transform.colorTransform;
            glyph.transform.colorTransform = FrozenPlaces.tint();
         }
         else
         {
            glyph.transform.colorTransform = this._ownGlyph;
            _ownGlyph = null;
         }
         this.paintLabel();
      }

      public function get frozen() : Boolean
      {
         return this._ownGlyph != null;
      }

      // The class's glyph: the upper of the frame's two parts named renderer (under it its shadow), the
      // one the squad's icons have in green.
      private function get glyph() : DisplayObject
      {
         var glyph:DisplayObject = null;
         for(var i:int = 0; i < numChildren; i++)
         {
            if(getChildAt(i).name == "renderer")
            {
               glyph = getChildAt(i);
            }
         }
         return glyph;
      }

      // Artifact Hunt: the carrier's dot in the artefact's light (CarryRing).
      public function set carrying(value:Boolean) : void
      {
         CarryRing.mark(this,value);
      }

      public function get carrying() : Boolean
      {
         return CarryRing.marked(this);
      }

      // Freeze Tag: a revealed enemy's dot (Minimap: a team's last one fighting) marked red
      // (RevealedMark); his name, if shown, kept over the mark.
      public function set revealed(value:Boolean) : void
      {
         if(value == this.revealed)
         {
            return;
         }
         RevealedMark.mark(this,value);
         if(this._label && this._label.parent == this)
         {
            this.placeLabel();
         }
      }

      public function get revealed() : Boolean
      {
         return RevealedMark.marked(this);
      }

      // The name centred just over what the icon draws (its name left out).
      private function placeLabel() : void
      {
         var icon:Rectangle = new Rectangle();
         for(var i:int = 0; i < numChildren; i++)
         {
            if(getChildAt(i) != this._label)
            {
               icon = icon.union(getChildAt(i).getBounds(this));
            }
         }
         this._label.x = -this._label.width / 2;
         this._label.y = icon.top - this._label.height + 2;
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
            this.paintLabel();
         }
         if(this._label.text != name || this._label.parent != this)
         {
            if(this._label.parent == this)
            {
               removeChild(this._label);
            }
            this._label.text = name;
            this.placeLabel();
            addChild(this._label);
         }
         this._label.visible = true;
      }

      // The label's colour in its own format (a copy: each text set later takes it): the ice blue while
      // frozen, else the name's.
      private function paintLabel() : void
      {
         if(!this._label)
         {
            return;
         }
         var format:TextFormat = this._label.defaultTextFormat;
         format.color = this.frozen ? FrozenPlaces.COLOR : this.nameColor;
         this._label.defaultTextFormat = format;
         this._label.textColor = uint(format.color);
      }

      // The name's colour: white; an enemy's red (PlayerRed), as the game's names over heads.
      protected function get nameColor() : uint
      {
         return 0xFFFFFF;
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
