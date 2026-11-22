package survarium.player
{
   import flash.display.DisplayObject;
   import flash.display.Sprite;
   import flash.events.MouseEvent;
   import flash.geom.Rectangle;
   import flash.utils.getTimer;
   import scaleform.clik.core.UIComponent;
   import survarium.constants.UiSound;
   import survarium.data.LocalizationData;
   import survarium.rooms.RoomsLook;
   import flash.external.ExternalInterface;

   // Artifact Container's slot (artefact-skills.md, "Artifact Container (522)"): with the skill,
   // the quick slot it turns into an artefact container is drawn with a yellow outline, as the
   // container or as the pocket it was, and a click switches it (srp's word, the presets list's
   // `K` row, says which it is; LocalPlayerProfile.containerSlot). The one owner of that state:
   // the paper doll places it (LocalPaperDoll.handleQuickSlots) and hands it the slot's clicks.
   //
   // - As the container (a locked slot takes no mouse): the mark takes the hover (the hint) and
   //   the click itself.
   // - As the pocket: the slot keeps the mouse (drops, its item); the paper doll forwards its
   //   hover and its click. A click on it while it holds an item is the item's, as any pocket's:
   //   it is made a container only empty (the owner's pick of 2026-11-22), so a stray click
   //   never moves an item.
   public class ContainerSlotMark extends Sprite
   {

      // The kit's slot as srp says it (ContainerSlot::code; 0: no such slot, no skill).
      public static const NONE:int = 0;

      public static const CONTAINER:int = 1;

      public static const POCKET:int = 2;

      // The owner's pick of 2026-11-22 (his word: "yellow contours"): the lobby's own yellow, the
      // Rooms tab's and the player's room's (RoomsLook.YELLOW, 0xFFD23F). RoomsLook.GOLD
      // (0xE0A64E) is the "chosen" colour, the other one he may compare.
      public static const COLOUR:uint = RoomsLook.YELLOW;

      private static const LINE:Number = 2;

      // A click's answer is the list with the `K` row; none in this long: clicks are taken again.
      private static const PENDING_MS:int = 3000;

      private var _state:int = NONE;

      private var _pendingAt:int = -1;

      // The slot it is placed on, null: none shown, and whether it holds an item.
      private var _slot:DisplayObject;

      private var _holds:Boolean = false;

      // The hint, the game's tooltip; its area the hover and click area as the container.
      private var _tip:ReasonsTip;

      // ask(code): the server asked to make the kit's slot that one (LocalPlayerProfile).
      private var _ask:Function;

      public function ContainerSlotMark()
      {
         super();
         visible = false;
         this._tip = new ReasonsTip();
         addChild(this._tip);
         this._tip.validateNow();
         this._tip.tooltipRendererName = "SrvProfileTipRendererStyled";
         addEventListener(MouseEvent.CLICK,this.onClick);
      }

      // The slot it is on; null: none.
      public function get slot() : DisplayObject
      {
         return this._slot;
      }

      // srp's word for the kit shown, and how to ask it for another: a click waits no longer.
      public function told(state:int, ask:Function) : void
      {
         this._state = state == CONTAINER || state == POCKET ? state : NONE;
         this._ask = ask;
         this._pendingAt = -1;
      }

      // The quick slot (13 + i) the skill's slot is among `pockets` pockets and `containers`
      // containers after them (PaperDollBase's counts): the first container as the container,
      // the last pocket as the pocket; -1: none shown.
      public function slotIndex(pockets:int, containers:int) : int
      {
         if(this._state == CONTAINER && containers >= 1)
         {
            return pockets;
         }
         if(this._state == POCKET && pockets >= 1)
         {
            return pockets - 1;
         }
         return -1;
      }

      // On `slot` (null: hidden), which holds an item or not; its bounds in the paper doll's
      // units.
      public function place(slot:DisplayObject, holds:Boolean) : void
      {
         this._slot = slot;
         this._holds = holds;
         if(slot == null || parent == null)
         {
            visible = false;
            this._tip.hideNow();
            return;
         }
         var at:Rectangle = slot.getBounds(parent);
         x = Math.round(at.x);
         y = Math.round(at.y);
         var w:Number = Math.round(at.width);
         var h:Number = Math.round(at.height);
         graphics.clear();
         graphics.lineStyle(LINE,COLOUR,1,true);
         graphics.drawRect(LINE / 2,LINE / 2,w - LINE,h - LINE);
         this._tip.graphics.clear();
         // Alpha 0 is not hit-tested in Scaleform: 0.01.
         this._tip.graphics.beginFill(0,0.01);
         this._tip.graphics.drawRect(0,0,w,h);
         this._tip.graphics.endFill();
         this._tip.anchor = slot;
         this._tip.tipData = {"name":this.hint()};
         // As the container the mark takes the mouse; as the pocket the slot keeps it.
         var container:Boolean = this._state == CONTAINER;
         mouseEnabled = mouseChildren = container;
         visible = true;
      }

      // The slot's hover as the pocket (the paper doll's): the hint, while it is empty.
      public function hover(over:Boolean) : void
      {
         if(over && this._state == POCKET && !this._holds && visible)
         {
            this._tip.tipData = {"name":this.hint()};
            this._tip.showNow();
         }
         else
         {
            this._tip.hideNow();
         }
      }

      // The slot clicked as the pocket (the paper doll's).
      public function slotClicked() : void
      {
         this.clicked();
      }

      private function onClick(e:MouseEvent) : void
      {
         this.clicked();
      }

      // What a click asks: the container a pocket, an empty pocket the container again; an item
      // in the pocket keeps it as it is. One ask at a time.
      private function clicked() : void
      {
         // The paper doll greyed (the kits locked: in a match, on the way to one): no change.
         var doll:UIComponent = parent as UIComponent;
         if(!visible || this._ask == null || doll != null && !doll.enabled)
         {
            return;
         }
         if(this._pendingAt >= 0 && getTimer() - this._pendingAt < PENDING_MS)
         {
            return;
         }
         var wanted:int = NONE;
         if(this._state == CONTAINER)
         {
            wanted = POCKET;
         }
         else if(this._state == POCKET && !this._holds)
         {
            wanted = CONTAINER;
         }
         if(wanted == NONE)
         {
            return;
         }
         this._pendingAt = getTimer();
         this._tip.hideNow();
         try
         {
            ExternalInterface.call("sound_play",UiSound.CLICK);
         }
         catch(err:Error)
         {
         }
         this._ask(wanted);
      }

      // What a click would make it.
      private function hint() : String
      {
         return LocalizationData.getName(this._state == CONTAINER ? "st_srp_loadout_slot_to_pocket" : "st_srp_loadout_slot_to_container");
      }
   }
}
