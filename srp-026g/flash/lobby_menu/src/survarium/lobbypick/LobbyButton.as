package survarium.lobbypick
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.utils.getDefinitionByName;
   import survarium.buttons.ButtonClick;

   // The lobby button (the player panel's, in the premium button's place; docs/notes/lobby-scenes.md):
   // the panel's money-box frame without anything of a money box. A code-made button (no timeline,
   // so no state frame places a coin, an amount or a hint again): its back is the money boxes' own
   // piece for the state (btnBackUp, btnBackHover, btnBackDown, btnBackDisabled, the pieces
   // SrvButtonTop's frames place), stretched to the box; its label the top menu's look (Calibri
   // bold, its colour, upper case), a little larger than the amounts, centred both ways by its
   // measured size. No tooltip and no data: it only opens the lobby window (ButtonEvent.CLICK).
   public class LobbyButton extends ButtonClick
   {
      private static const LABEL_SIZE:int = 16;
      private static const LABEL_COLOUR:uint = 0x9BADC1;

      // the back piece of each state CLIK can ask for (ButtonClick's state map)
      private static const BACKS:Object = {
         up:"btnBackUp", out:"btnBackUp", toggle:"btnBackUp", kb_selecting:"btnBackUp", kb_release:"btnBackUp",
         over:"btnBackHover", release:"btnBackHover", selecting:"btnBackHover",
         down:"btnBackDown", kb_down:"btnBackDown",
         disabled:"btnBackDisabled"
      };

      private var _w:Number;
      private var _h:Number;
      private var _back:MovieClip;
      private var _shown:String;
      private var _caption:TextField;

      public function LobbyButton(w:Number, h:Number)
      {
         super();
         this._w = w;
         this._h = h;
         this._caption = new TextField();
         this._caption.name = "lobbyLabel";
         this._caption.defaultTextFormat = new TextFormat("Calibri",LABEL_SIZE,LABEL_COLOUR,true);
         this._caption.embedFonts = true;
         this._caption.selectable = false;
         this._caption.mouseEnabled = false;
         this._caption.multiline = false;
         this._caption.wordWrap = false;
         this._caption.autoSize = TextFieldAutoSize.LEFT;
         addChild(this._caption);
         this.back("up");
         // CLIK measured the button while it was still empty (UIComponent.initialize, 0 x 0) and
         // its draw would size it to that: the button draws its own box at its size instead.
         this.preventAutosizing = true;
         setSize(w,h);
      }

      // The label's text (the dictionary's, upper case), centred in the box.
      public function set text(value:String) : void
      {
         this._caption.text = value.toUpperCase();
         this._caption.x = Math.round((this._w - this._caption.width) / 2);
         this._caption.y = Math.round((this._h - this._caption.height) / 2);
      }

      // The back piece the button shows now (its class name).
      public function get shown() : String
      {
         return this._shown;
      }

      // The box's size (the premium button's), which the back pieces are stretched to.
      public function get boxWidth() : Number
      {
         return this._w;
      }

      public function get boxHeight() : Number
      {
         return this._h;
      }

      // The back piece of a state CLIK can ask for (its class name), null for none.
      public static function backOf(state:String) : String
      {
         return BACKS[state] as String;
      }

      // Every state CLIK can ask for (ButtonClick's state map).
      public static function get states() : Array
      {
         var out:Array = [];
         for(var state:String in BACKS)
         {
            out.push(state);
         }
         return out.sort();
      }

      override protected function setState(state:String) : void
      {
         super.setState(state);
         this.back(state);
      }

      private function back(state:String) : void
      {
         var name:String = backOf(state);
         if(name == null || name == this._shown)
         {
            return;
         }
         var cls:Class = getDefinitionByName(name) as Class;
         var piece:MovieClip = new cls() as MovieClip;
         piece.width = this._w;
         piece.height = this._h;
         if(this._back != null)
         {
            removeChild(this._back);
         }
         addChildAt(piece,0);
         this._back = piece;
         this._shown = name;
      }
   }
}
