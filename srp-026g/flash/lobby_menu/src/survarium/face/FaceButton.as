package survarium.face
{
   import flash.events.Event;
   import scaleform.clik.events.ButtonEvent;
   import survarium.controls.PanelButton;
   import survarium.controls.Transient;
   import survarium.data.LocalizationData;

   // The Face button (face-choice.md, Lobby): the player panel's, left of the Lobby button and as
   // it is (PanelButton), "FACE" (st_srp_face). Shown when srp has said the game data lists more
   // than one face. A click opens the faces' chooser under it (FaceChooser), a transient window
   // (docs/notes/transient-windows.md): a click on the button again, a press outside it or Esc
   // closes it. The button owns its chooser; the chooser is drawn in the panel, under the button.
   public class FaceButton extends PanelButton
   {

      // Between the button and its chooser.
      private static const DROP:int = 4;

      private var _choice:FaceChoice;

      private var _chooser:FaceChooser;

      public function FaceButton(w:Number, h:Number)
      {
         super(w,h);
         name = "btn_face";
         this._choice = FaceChoice.shared();
         this._chooser = new FaceChooser(this._choice);
         this._choice.addEventListener(Event.CHANGE,this.onChoice);
         addEventListener(ButtonEvent.CLICK,this.onClick);
         LocalizationData.addCallback(this.onLabel,"st_srp_face");
         this.onChoice(null);
      }

      // Its chooser (open or not).
      public function get chooser() : FaceChooser
      {
         return this._chooser;
      }

      public function get isOpen() : Boolean
      {
         return this._chooser.parent != null;
      }

      private function onLabel(value:String) : void
      {
         text = value;
      }

      // Shown with more than one face; hidden, its chooser closes.
      private function onChoice(e:Event) : void
      {
         visible = this._choice.count > 1;
         if(!visible)
         {
            this.close();
         }
      }

      private function onClick(e:ButtonEvent) : void
      {
         if(this.isOpen)
         {
            this.close();
         }
         else
         {
            this.open();
         }
      }

      private function open() : void
      {
         if(parent == null || !visible)
         {
            return;
         }
         this._chooser.x = x;
         this._chooser.y = y + boxHeight + DROP;
         parent.addChild(this._chooser);
         Transient.open(this._chooser,this,this.close);
      }

      private function close() : void
      {
         if(!this.isOpen)
         {
            return;
         }
         // home first (off the transient layer), then out of the panel
         Transient.closed(this._chooser);
         if(this._chooser.parent != null)
         {
            this._chooser.parent.removeChild(this._chooser);
         }
      }
   }
}
