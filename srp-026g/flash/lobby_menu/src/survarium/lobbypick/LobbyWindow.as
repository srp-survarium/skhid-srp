package survarium.lobbypick
{
   import flash.display.Bitmap;
   import flash.display.BitmapData;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.geom.Point;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import flash.utils.getDefinitionByName;
   import scaleform.gfx.MouseEventEx;
   import survarium.data.LocalizationData;

   // The lobby window (exe patch 81, docs/notes/lobby-scenes.md): four cards, a picture and a name
   // each. A click on one picks that lobby: srp is told (its 3B at PICK_BASE + id, the lobby's id),
   // gives the pick back in 0xA1, and the exe shows that lobby behind its fade. The pick is marked.
   // What it shows the exe tells it (`state`, from root.srp_lobby_state). Presses are taken by the window itself and found by
   // where they are (Scaleform gives presses on a container's children to the container).
   public class LobbyWindow extends Sprite
   {
      public static const PICK_BASE:int = 2095000000;

      // the cards in id order, 1 to 4: their names (dictionary keys) and pictures (bitmaps/)
      private static const NAMES:Array = ["st_srp_lobby_room","st_srp_lobby_basement","st_srp_lobby_bunker","st_srp_lobby_hangar"];
      private static const PICTURES:Array = ["SrpLobbyRoom","SrpLobbyBasement","SrpLobbyBunker","SrpLobbyHangar"];

      private static const CARD_W:Number = 192;
      private static const CARD_H:Number = 108;
      private static const GAP:Number = 12;
      private static const PAD:Number = 16;
      private static const TITLE_H:Number = 34;
      private static const NAME_H:Number = 26;
      private static const GOLD:uint = 0xE0A64E;
      private static const LIGHT:uint = 0xE8EEF2;

      // what the exe said: the pick (id)
      private var _pick:int = 1;
      // a click's pick, until the exe says it
      private var _asked:int = 0;

      private var _cards:Array = [];
      private var _marks:Array = [];
      private var _names:Array = [];
      private var _title:TextField;

      // what closes it (the lobby's: the modal left)
      public var onClose:Function;

      public function LobbyWindow(font:String, bold:Object)
      {
         super();
         name = "srpLobbyWindow";
         var w:Number = PAD * 2 + CARD_W * 4 + GAP * 3;
         var h:Number = TITLE_H + CARD_H + NAME_H + PAD;
         graphics.beginFill(0x0E1214,0.96);
         graphics.drawRect(0,0,w,h);
         graphics.endFill();
         graphics.lineStyle(1,0x3A4248);
         graphics.drawRect(0,0,w,h);
         this._title = this.text(font,bold,"",PAD,8,w - PAD * 2,16,LIGHT,"left");
         for(var i:int = 0; i < 4; i++)
         {
            var x:Number = PAD + i * (CARD_W + GAP);
            var y:Number = TITLE_H;
            var cls:Class = getDefinitionByName(PICTURES[i]) as Class;
            var picture:Bitmap = new Bitmap(new cls() as BitmapData);
            picture.smoothing = true;
            picture.x = x;
            picture.y = y;
            picture.width = CARD_W;
            picture.height = CARD_H;
            addChild(picture);
            this._cards.push(picture);
            var mark:Shape = new Shape();
            mark.x = x;
            mark.y = y;
            addChild(mark);
            this._marks.push(mark);
            this._names.push(this.text(font,bold,"",x,y + CARD_H + 4,CARD_W,14,LIGHT,"center"));
         }
         LocalizationData.addCallback(this.labels,"st_srp_lobby_title");
         addEventListener(MouseEvent.MOUSE_DOWN,this.onDown);
         this.redraw();
      }

      private function text(font:String, bold:Object, value:String, x:Number, y:Number, w:Number, size:Number, color:uint, align:String) : TextField
      {
         var tf:TextField = new TextField();
         var format:TextFormat = new TextFormat(font,size,color,bold);
         format.align = align;
         tf.defaultTextFormat = format;
         tf.embedFonts = true;
         tf.selectable = false;
         tf.mouseEnabled = false;
         tf.x = x;
         tf.y = y;
         tf.width = w;
         tf.height = size + 8;
         tf.text = value;
         addChild(tf);
         return tf;
      }

      private function labels(title:String) : void
      {
         this._title.text = title;
         for(var i:int = 0; i < 4; i++)
         {
            TextField(this._names[i]).text = LocalizationData.getName(NAMES[i]);
         }
      }

      // The exe's state (root.srp_lobby_state): the pick, the lobby shown.
      public function state(pick:uint, shown:uint) : void
      {
         this._pick = int(pick);
         if(this._asked == this._pick)
         {
            this._asked = 0;
         }
         this.redraw();
      }

      private function marked() : int
      {
         return this._asked != 0 ? this._asked : this._pick;
      }

      private function redraw() : void
      {
         for(var i:int = 0; i < 4; i++)
         {
            var id:int = i + 1;
            var mark:Shape = this._marks[i];
            mark.graphics.clear();
            if(id == this.marked())
            {
               mark.graphics.lineStyle(3,GOLD);
               mark.graphics.drawRect(0,0,CARD_W,CARD_H);
            }
            TextField(this._names[i]).textColor = id == this.marked() ? GOLD : LIGHT;
         }
      }

      // A press: the card under it picked (not the pick already), and the window closed.
      private function onDown(e:MouseEvent) : void
      {
         var ex:MouseEventEx = e as MouseEventEx;
         if(ex != null && ex.buttonIdx != MouseEventEx.LEFT_BUTTON)
         {
            return;
         }
         var at:Point = globalToLocal(new Point(e.stageX,e.stageY));
         for(var i:int = 0; i < 4; i++)
         {
            var x:Number = PAD + i * (CARD_W + GAP);
            if(at.x < x || at.x >= x + CARD_W || at.y < TITLE_H || at.y >= TITLE_H + CARD_H + NAME_H)
            {
               continue;
            }
            var id:int = i + 1;
            if(id != this.marked())
            {
               this._asked = id;
               this.redraw();
               ExternalInterface.call("request_players_range",PICK_BASE + id,1);
            }
            if(this.onClose != null)
            {
               this.onClose();
            }
            return;
         }
      }
   }
}
