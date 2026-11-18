package survarium.lobbypick
{
   import flash.display.Bitmap;
   import flash.display.BitmapData;
   import flash.display.DisplayObject;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.utils.getDefinitionByName;
   import survarium.data.LocalizationData;
   import survarium.rooms.RoomsLook;

   // The lobby window (exe patch 81, docs/notes/lobby-scenes.md): six cards as the rooms' map vote
   // draws its own (RoomsRoom), the lobby's render over the whole card, its name on a dark band. A
   // click on one picks that lobby: srp is told (its 3B at PICK_BASE + id, the lobby's id), gives
   // the pick back in 0xA1, and the exe shows that lobby behind its fade. The pick has the gold frame.
   // What it shows the exe tells it (`state`, from root.srp_lobby_state). A press Scaleform gives the
   // window itself goes to the card under it (RoomsLook.route), as in the rooms window.
   public class LobbyWindow extends Sprite
   {
      public static const PICK_BASE:int = 2095000000;

      // the cards in id order, 1 to 6: their names (dictionary keys) and pictures (bitmaps/)
      private static const NAMES:Array = ["st_srp_lobby_room","st_srp_lobby_basement","st_srp_lobby_bunker","st_srp_lobby_hangar","st_srp_lobby_bar","st_srp_lobby_range"];
      private static const PICTURES:Array = ["SrpLobbyRoom","SrpLobbyBasement","SrpLobbyBunker","SrpLobbyHangar","SrpLobbyBar","SrpLobbyRange"];

      // a vote card's size (RoomsRoom: a third of the rooms window less its margins, 190 high), three
      // to a row
      private static const COLUMNS:int = 3;
      private static const CARD_W:int = 280;
      private static const CARD_H:int = 190;
      private static const GAP:int = 14;
      private static const PAD:int = 16;
      private static const TITLE_H:int = 44;

      // what the exe said: the pick (id)
      private var _pick:int = 1;
      // a click's pick, until the exe says it
      private var _asked:int = 0;

      private var _look:RoomsLook;
      private var _frames:Array = [];
      private var _names:Array = [];
      private var _title:TextField;

      // what closes it (the lobby's: the modal left)
      public var onClose:Function;

      public function LobbyWindow(font:String, bold:Object)
      {
         super();
         name = "srpLobbyWindow";
         this._look = new RoomsLook(this,this,font,bold);
         var rows:int = Math.ceil(NAMES.length / COLUMNS);
         var w:Number = PAD * 2 + CARD_W * COLUMNS + GAP * (COLUMNS - 1);
         var h:Number = TITLE_H + CARD_H * rows + GAP * (rows - 1) + PAD;
         graphics.beginFill(0x0E1214,0.96);
         graphics.drawRect(0,0,w,h);
         graphics.endFill();
         graphics.lineStyle(1,0x3A4248);
         graphics.drawRect(0,0,w,h);
         this._title = this._look.text(this,"",PAD,14,w - PAD * 2,16,RoomsLook.LIGHT);
         for(var i:int = 0; i < NAMES.length; i++)
         {
            var cell:Sprite = new Sprite();
            cell.x = PAD + i % COLUMNS * (CARD_W + GAP);
            cell.y = TITLE_H + int(i / COLUMNS) * (CARD_H + GAP);
            var cls:Class = getDefinitionByName(PICTURES[i]) as Class;
            cell.addChild(new Bitmap(RoomsLook.cover(new cls() as BitmapData,CARD_W,CARD_H)));
            var shade:Shape = new Shape();
            shade.graphics.beginFill(0,0.01);
            shade.graphics.drawRect(0,0,CARD_W,CARD_H);
            shade.graphics.endFill();
            shade.graphics.beginFill(0,0.62);
            shade.graphics.drawRect(0,CARD_H - 58,CARD_W,58);
            shade.graphics.endFill();
            cell.addChild(shade);
            var frame:Shape = new Shape();
            cell.addChild(frame);
            this._frames.push(frame);
            this._names.push(this._look.text(cell,"",12,CARD_H - 54,CARD_W - 24,21,0xFFFFFF));
            cell.buttonMode = true;
            cell.mouseChildren = false;
            cell.name = String(i + 1);
            cell.addEventListener(MouseEvent.CLICK,this.onCard,false,0,true);
            this._look.route(cell,this.onRouted);
            addChild(cell);
         }
         LocalizationData.addCallback(this.labels,"st_srp_lobby_title");
         addEventListener(MouseEvent.MOUSE_DOWN,this._look.onContentDown);
         this.redraw();
      }

      private function labels(title:String) : void
      {
         this._title.text = title.toUpperCase();
         for(var i:int = 0; i < NAMES.length; i++)
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

      // The pick framed gold (3 px), the others as a vote card is (1 px).
      private function redraw() : void
      {
         for(var i:int = 0; i < NAMES.length; i++)
         {
            var mine:Boolean = i + 1 == this.marked();
            var frame:Shape = this._frames[i];
            frame.graphics.clear();
            frame.graphics.lineStyle(mine ? 3 : 1,mine ? RoomsLook.GOLD : 0x26323C);
            frame.graphics.drawRect(0,0,CARD_W,CARD_H);
         }
      }

      private function onCard(e:Event) : void
      {
         this.press(e.currentTarget as DisplayObject);
      }

      private function onRouted(control:DisplayObject, e:MouseEvent) : void
      {
         this.press(control);
      }

      // A card pressed: its lobby picked (not the pick already), and the window closed.
      private function press(card:DisplayObject) : void
      {
         RoomsLook.clickSound();
         var id:int = int(card.name);
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
      }
   }
}
