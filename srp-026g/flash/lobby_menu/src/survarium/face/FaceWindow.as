package survarium.face
{
   import flash.display.Bitmap;
   import flash.display.BitmapData;
   import flash.display.DisplayObject;
   import flash.display.GradientType;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.geom.Matrix;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import survarium.data.IconContainer;
   import survarium.data.LocalizationData;
   import survarium.rooms.CardWindow;
   import survarium.rooms.RoomsLook;

   // The faces' window (face-choice.md, Lobby), opened by the Face button as the lobby window is by
   // the Lobby button, and drawn as it is (CardWindow): a card a face, its portrait over the card,
   // "Face <n>" on the dark band (st_srp_face_card), the account's face framed gold. A press on a
   // card chooses that face at once (FaceChoice.pick) and closes the window, which shows the lobby's
   // character, wearing it when srp's account comes (the exe).
   //
   // A face's portrait is the lobby's equipment atlas cell the faces pack draws for it (its
   // heads[i].icon, faces-pack.md: row i + 1 of ICON_COLUMN), its middle across the card; an atlas
   // without that cell shows the face's number instead.
   public class FaceWindow extends CardWindow
   {

      // The atlas's column of the faces (resource-porter's faces_026::ICON_COLUMN), from 1.
      public static const ICON_COLUMN:int = 16;

      // An equipment cell (171 x 83): the head is its middle HEAD_W px, which spans the card.
      private static const ICON_W:int = 171;

      private static const HEAD_W:int = 80;

      // The cell drawn this far up: the chin clear of the band, a crest still whole.
      private static const PORTRAIT_UP:int = 4;

      // A portrait card, the cell 1.75 times: the head over the band, the neck under it.
      public static const CARD_W:int = 140;

      public static const CARD_H:int = 180;

      private static const BAND_H:int = 40;

      private static const NAME_SIZE:int = 18;

      private static const NUMBER_SIZE:int = 56;

      // The most cards a row: more faces make rows of as many as even them out.
      private static const ROW_MOST:int = 6;

      private var _choice:FaceChoice;

      public function FaceWindow()
      {
         super(CARD_W,CARD_H,BAND_H,NAME_SIZE);
         name = "srpFaceWindow";
         this._choice = FaceChoice.shared();
         this._choice.addEventListener(Event.CHANGE,this.onChoice);
         this.rebuild();
         LocalizationData.addCallback(this.labels,"st_srp_face");
      }

      // The cards in rows of `columns`.
      public static function columnsOf(count:int) : int
      {
         var rows:int = Math.ceil(count / ROW_MOST);
         return Math.max(1,Math.ceil(count / Math.max(1,rows)));
      }

      // The cards made again when the count of faces changed; the pick framed otherwise. Nothing to
      // choose (the button hidden): closed.
      private function onChoice(e:Event) : void
      {
         if(this._choice.count <= 1)
         {
            close();
         }
         if(this._choice.count != count)
         {
            this.rebuild();
         }
         else
         {
            mark(this._choice.shown);
         }
      }

      private function rebuild() : void
      {
         var font:TextFormat = gameFont();
         var pictures:Array = [];
         for(var i:int = 0; i < this._choice.count; i++)
         {
            pictures.push(this.picture(i,font));
         }
         build(pictures,columnsOf(pictures.length));
         this.labels(LocalizationData.getName("st_srp_face"));
         mark(this._choice.shown);
      }

      private function labels(title:String) : void
      {
         setTitle(title.toUpperCase());
         for(var i:int = 0; i < count; i++)
         {
            setLabel(i,LocalizationData.format("st_srp_face_card",i + 1));
         }
      }

      // The face's portrait on a dark backdrop; its number without an atlas cell.
      private function picture(face:int, font:TextFormat) : DisplayObject
      {
         var card:Sprite = new Sprite();
         var m:Matrix = new Matrix();
         m.createGradientBox(CARD_W,CARD_H,Math.PI / 2);
         card.graphics.beginGradientFill(GradientType.LINEAR,[0x2C353C,0x0B0F12],[1,1],[0,255],m);
         card.graphics.drawRect(0,0,CARD_W,CARD_H);
         card.graphics.endFill();
         var icons:IconContainer = IconContainer.getInstance("equipment");
         if(icons != null && icons.columns >= ICON_COLUMN && face < icons.rows)
         {
            var cell:BitmapData = icons.iconData((ICON_COLUMN - 1) * icons.rows + face);
            var scale:Number = CARD_W / HEAD_W;
            var at:Matrix = new Matrix();
            at.scale(scale,scale);
            at.translate(Math.round((CARD_W - ICON_W * scale) / 2),-PORTRAIT_UP);
            var portrait:BitmapData = new BitmapData(CARD_W,CARD_H,true,0);
            portrait.draw(cell,at,null,null,null,true);
            card.addChild(new Bitmap(portrait));
            return card;
         }
         var number:TextField = new TextField();
         number.defaultTextFormat = new TextFormat(font.font,NUMBER_SIZE,RoomsLook.GREY,font.bold);
         number.embedFonts = true;
         number.selectable = false;
         number.mouseEnabled = false;
         number.autoSize = TextFieldAutoSize.LEFT;
         number.text = String(face + 1);
         number.x = Math.round((CARD_W - number.width) / 2);
         number.y = Math.round((CARD_H - BAND_H - number.height) / 2);
         card.addChild(number);
         return card;
      }

      // A card pressed: that face chosen (FaceChoice: not the face shown already), and the window
      // closed.
      override protected function pressed(card:int) : void
      {
         this._choice.pick(card);
         close();
      }
   }
}
