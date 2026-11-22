package survarium.player
{
   import flash.display.DisplayObject;
   import flash.display.GradientType;
   import flash.display.Graphics;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.geom.Matrix;
   import flash.geom.Rectangle;
   import flash.utils.getTimer;
   import scaleform.clik.core.UIComponent;
   import scaleform.gfx.MouseEventEx;
   import survarium.constants.UiSound;
   import survarium.controls.PlayerInfo;
   import survarium.data.LocalizationData;
   import survarium.rooms.RoomsLook;

   // A perk's slot (artefact-skills.md, "The slot a pocket again, by kit"): a skill that turns one
   // slot of a kit into another, which each kit takes or declines by a click. srp's word, the
   // presets list's `K` row, says which (LocalPlayerProfile.takePerks); the one owner of that state
   // for the kit shown. The paper doll places it on the perk's slot (LocalPaperDoll.placeMarks),
   // hands it the slot's own hover and clicks.
   //
   // - Artifact Container (CONTAINER): taken, the container (a locked slot: the mark takes the
   //   mouse itself); declined, the pocket it was.
   // - Deep Pockets or Supply Pocket (WEAPON): taken, the pocket it adds, the second weapon slot
   //   closed by the red lock (SlotLock); declined, the second weapon slot, the pocket closed so.
   //   A press on the red lock switches too (blockedClicked).
   // - The marked slot has a soft edge of the lobby's yellow: all round on a container or a
   //   weapon slot, glowing from the bottom up on a pocket. A click on it while it holds an item
   //   is the item's (the owner's pick of 2026-11-22: a stray click never moves an item; on the
   //   open second weapon slot holding a weapon, its menu, the red pocket switching then). The red
   //   lock's slot is empty, so its click always switches, and what
   //   the yellow slot held comes off the kit (it stays owned): the item goes back with a sound and
   //   a shake of its slot (DISPLACED, the paper doll's; the owner: "some good sound design to say
   //   that, or maybe some shaking").
   // - The switch is the right mouse button (the owner, 2026-11-22: "lets make it so rmb switches
   //   not lmb"): the left one does what the slot always did (the shop, an item chosen, a drag).
   //   A right click on a slot holding an item is its menu (LocalPaperDoll.onMouseItemClick),
   //   where no switch is made either.
   // - A click asks srp (one switch at a time, LocalPlayerProfile's PerkSwitch); its answer, the
   //   kit's account and srp's word together, is drawn at once, the mark's look cross-faded. The
   //   mark shows srp's word only, never a guess, so the kit is drawn old or new, never half.
   public class PerkSlotMark extends Sprite
   {

      // The perks, SlotPerk::index.
      public static const CONTAINER:int = 0;

      public static const WEAPON:int = 1;

      // The kit's slot as srp says it (PerkSlot::code; 0: no such perk).
      public static const NONE:int = 0;

      public static const TAKEN:int = 1;

      public static const DECLINED:int = 2;

      // The event of a switch that sends the yellow slot's item back to the inventory.
      public static const DISPLACED:String = "perkSlotDisplaced";

      // The hover's fade.
      private static const FADE_MS:Number = 130;

      private var _perk:int;

      // srp's word, the look shown.
      private var _shown:int = NONE;


      // The slot it is on, null: none, its size drawn, and whether it holds an item.
      private var _slot:DisplayObject;

      private var _w:Number = 0;

      private var _h:Number = 0;

      private var _holds:Boolean = false;

      // The slot a switch closes (the last pocket, or the second weapon slot; null: none), and
      // whether it holds an item, which the switch sends back.
      private var _closing:DisplayObject;

      private var _closingHolds:Boolean = false;

      // Each state's look at rest and what the hover adds ([NONE] unused), and how much of each is
      // shown now, eased toward its target a frame at a time.
      private var _rest:Array = [null,new Shape(),new Shape()];

      private var _lift:Array = [null,new Shape(),new Shape()];

      private var _show:Array = [0,0,0];

      private var _over:Number = 0;

      private var _overWanted:Boolean = false;

      private var _frameAt:int = -1;

      // The hint, the game's tooltip; its area the hover and click area where the mark takes the
      // mouse.
      private var _tip:ReasonsTip;

      // ask(perk, state): the server asked to make the kit's slot that one (LocalPlayerProfile).
      private var _ask:Function;

      public function PerkSlotMark(perk:int)
      {
         super();
         this._perk = perk;
         visible = false;
         for each(var state:int in [TAKEN,DECLINED])
         {
            addChild(this._rest[state]);
            addChild(this._lift[state]);
            this._rest[state].alpha = this._lift[state].alpha = 0;
         }
         this._tip = new ReasonsTip();
         addChild(this._tip);
         this._tip.validateNow();
         this._tip.tooltipRendererName = "SrvProfileTipRendererStyled";
         addEventListener(MouseEvent.CLICK,this.onClick);
         addEventListener(MouseEvent.ROLL_OVER,this.onOver);
         addEventListener(MouseEvent.ROLL_OUT,this.onOut);
      }

      // The slot it is on; null: none.
      public function get slot() : DisplayObject
      {
         return this._slot;
      }

      // The slot a switch closes; null: none.
      public function get closingSlot() : DisplayObject
      {
         return this._closing;
      }

      // The slot a switch from the state shown closes, and whether it holds an item (the paper
      // doll's, before place).
      public function closing(slot:DisplayObject, holds:Boolean) : void
      {
         this._closing = slot;
         this._closingHolds = slot != null && holds;
      }

      // The look shown: srp's word.
      public function get shown() : int
      {
         return this._shown;
      }

      // srp's word for the kit `kit` shown, and how to ask it for another (ask(perk, state):
      // whether it was asked). Shown at once, in the same frame as the slots it goes with (a
      // switch is drawn whole: only the hover fades).
      public function told(kit:uint, state:int, ask:Function) : void
      {
         this._ask = ask;
         this.show(state == TAKEN || state == DECLINED ? state : NONE,true);
      }

      // The paper doll's word whether a switch may start now (none while a slot shakes); none:
      // always.
      public var mayClick:Function;

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
         if(w != this._w || h != this._h)
         {
            this._w = w;
            this._h = h;
            this.drawLooks();
         }
         this._tip.anchor = slot;
         this._tip.tipData = {"name":this.label()};
         mouseEnabled = mouseChildren = this.takesMouse(this._shown);
         visible = true;
      }

      // The hover: the look lifted; where the slot keeps the mouse (the paper doll's word) the
      // tip too, above the slot: the skill and what a right click does while it is empty; the
      // skill alone beside a held item's card (the owner, 2026-11-22: "pop up helper messages
      // even on items should say which skill this 'slot' corresponds to").
      public function hover(over:Boolean) : void
      {
         this._overWanted = over;
         this.fade();
         if(over && !this.takesMouse(this._shown) && visible)
         {
            this._tip.tipData = {"name":this._holds ? LocalizationData.format("st_srp_loadout_slot_skill",this.skillName()) : this.label()};
            this._tip.showNow();
         }
         else if(!over || !this.takesMouse(this._shown))
         {
            this._tip.hideNow();
         }
      }

      // The marked slot clicked, where it keeps the mouse (the paper doll's).
      public function slotClicked() : void
      {
         this.clicked(false);
      }

      // The red lock clicked (SlotLock): the slot the perk's state closes.
      public function blockedClicked() : void
      {
         this.clicked(true);
      }

      // The skill this slot is of, in the game's own names.
      public function skillName() : String
      {
         if(this._perk == CONTAINER)
         {
            return LocalizationData.getName("st_srp_artefact_522_name");
         }
         return LocalizationData.getName(PlayerInfo.supplyPocket ? "st_srp_medical_331_name" : "st_srp_deep_pockets_name");
      }

      // The tip of the slot while empty, and of the red locks: its skill and what a right click
      // does ("Deep Pockets — Right-click: second weapon").
      public function label() : String
      {
         return this.skillName() + " — " + this.hint();
      }

      // What a click switches to.
      public function hint() : String
      {
         var pocket:String = "st_srp_loadout_slot_to_pocket";
         if(this._perk == CONTAINER)
         {
            return LocalizationData.getName(this._shown == TAKEN ? pocket : "st_srp_loadout_slot_to_container");
         }
         return LocalizationData.getName(this._shown == TAKEN ? "st_srp_loadout_slot_to_weapon" : pocket);
      }

      private function onClick(e:MouseEvent) : void
      {
         if(rightClick(e))
         {
            this.clicked(false);
         }
      }

      // Whether a click is the right button's (Scaleform's own events say which).
      public static function rightClick(e:MouseEvent) : Boolean
      {
         return e is MouseEventEx && (e as MouseEventEx).buttonIdx == MouseEventEx.RIGHT_BUTTON;
      }

      private function onOver(e:MouseEvent) : void
      {
         this.hover(true);
      }

      private function onOut(e:MouseEvent) : void
      {
         this.hover(false);
      }

      // Where the mark itself takes the mouse: on Artifact Container's container, a locked slot.
      private function takesMouse(state:int) : Boolean
      {
         return this._perk == CONTAINER && state == TAKEN;
      }

      // The look of `state`'s slot: all round on a container or a weapon slot, from the bottom on
      // a pocket.
      private function allRound(state:int) : Boolean
      {
         return (this._perk == CONTAINER) == (state == TAKEN);
      }

      // What a click asks: the other state; on the yellow slot only while it holds nothing. Nothing
      // changes until srp's answer (LocalPlayerProfile draws it whole); one ask at a time.
      private function clicked(blocked:Boolean) : void
      {
         // The paper doll greyed (the kits locked: in a match, on the way to one): no change.
         var doll:UIComponent = parent as UIComponent;
         if(this._shown == NONE || this._ask == null || doll != null && !doll.enabled || this.mayClick != null && !this.mayClick())
         {
            return;
         }
         if(!blocked && (!visible || this._holds))
         {
            return;
         }
         var wanted:int = this._shown == TAKEN ? DECLINED : TAKEN;
         if(!this._ask(this._perk,wanted))
         {
            return;
         }
         // A switch that closes a slot holding an item: that item goes back (the slot it closes is
         // the perk's own: its pocket, or the second weapon slot).
         var displaced:Boolean = this._closingHolds;
         this._tip.hideNow();
         try
         {
            // An item sent back: the lobby's sound of an item taken (the movie has no other for
            // an item moved; the stock list, gameplay/ui_sounds 14 ammo_pick_up).
            ExternalInterface.call("sound_play",displaced ? UiSound.AMMO_PICK_UP : UiSound.CLICK);
         }
         catch(err:Error)
         {
         }
         if(displaced)
         {
            dispatchEvent(new Event(DISPLACED));
         }
      }

      // `state`'s look shown: at once, or cross-faded from the one before.
      private function show(state:int, now:Boolean) : void
      {
         this._shown = state;
         if(now)
         {
            for each(var s:int in [TAKEN,DECLINED])
            {
               this._show[s] = s == state ? 1 : 0;
            }
            this.apply();
         }
         this.fade();
      }

      // The fades go on a frame at a time (the movie's own frames) until each is where it goes.
      private function fade() : void
      {
         if(!hasEventListener(Event.ENTER_FRAME))
         {
            this._frameAt = getTimer();
            addEventListener(Event.ENTER_FRAME,this.onFrame);
         }
      }

      private function onFrame(e:Event) : void
      {
         var now:int = getTimer();
         var step:Number = Math.min(1,(now - this._frameAt) / FADE_MS);
         this._frameAt = now;
         var settled:Boolean = true;
         for each(var s:int in [TAKEN,DECLINED])
         {
            var target:Number = s == this._shown ? 1 : 0;
            this._show[s] = toward(this._show[s],target,step);
            settled = settled && this._show[s] == target;
         }
         var over:Number = this._overWanted ? 1 : 0;
         this._over = toward(this._over,over,step);
         settled = settled && this._over == over;
         this.apply();
         if(settled)
         {
            removeEventListener(Event.ENTER_FRAME,this.onFrame);
         }
      }

      private static function toward(value:Number, target:Number, step:Number) : Number
      {
         return value < target ? Math.min(target,value + step) : Math.max(target,value - step);
      }

      // Eased: smooth at both ends.
      private static function ease(t:Number) : Number
      {
         return t * t * (3 - 2 * t);
      }

      private function apply() : void
      {
         for each(var s:int in [TAKEN,DECLINED])
         {
            this._rest[s].alpha = ease(this._show[s]);
            this._lift[s].alpha = ease(this._show[s]) * ease(this._over);
         }
      }

      // Each state's look at rest and its hover's lift, for the slot's size: the soft edge (the
      // owner's pick of 2026-11-22), a 1 px line at half strength and the yellow glowing into the
      // slot from it, all round or from the bottom up (allRound). The hover brings the line to the
      // full yellow and the glow up. The glow is drawn as gradients: a GlowFilter would have
      // Scaleform render the mark into a bitmap of its own at every change of its alpha (each
      // frame of a fade); four gradients are four quads.
      private function drawLooks() : void
      {
         var w:Number = this._w;
         var h:Number = this._h;
         var edge:Array = [[0,0,w,1],[0,h - 1,w,1],[0,1,1,h - 2],[w - 1,1,1,h - 2]];
         var depth:Number = Math.round(Math.min(w,h) / 9);
         var rise:Number = Math.round(h / 2);
         for each(var s:int in [TAKEN,DECLINED])
         {
            for each(var g:Graphics in [this._rest[s].graphics,this._lift[s].graphics])
            {
               g.clear();
               var line:Number = g == this._rest[s].graphics ? RoomsLook.YELLOW_HALF : 1;
               for each(var p:Array in edge)
               {
                  g.beginFill(RoomsLook.YELLOW,line);
                  g.drawRect(p[0],p[1],p[2],p[3]);
                  g.endFill();
               }
               if(this.allRound(s))
               {
                  glow(g,1,1,w - 2,depth,Math.PI / 2);
                  glow(g,1,h - 1 - depth,w - 2,depth,-Math.PI / 2);
                  glow(g,1,1,depth,h - 2,0);
                  glow(g,w - 1 - depth,1,depth,h - 2,Math.PI);
               }
               else
               {
                  glow(g,1,h - 1 - rise,w - 2,rise,-Math.PI / 2);
               }
            }
         }
         // The hover and click area.
         this._tip.graphics.clear();
         // Alpha 0 is not hit-tested in Scaleform: 0.01.
         this._tip.graphics.beginFill(0,0.01);
         this._tip.graphics.drawRect(0,0,w,h);
         this._tip.graphics.endFill();
      }

      // The yellow from the glow's strength at one edge of the rectangle to nothing at the other,
      // along `angle` (0: from the left).
      private static function glow(g:Graphics, x:Number, y:Number, w:Number, h:Number, angle:Number) : void
      {
         var m:Matrix = new Matrix();
         m.createGradientBox(w,h,angle,x,y);
         g.beginGradientFill(GradientType.LINEAR,[RoomsLook.YELLOW,RoomsLook.YELLOW],[RoomsLook.YELLOW_GLOW,0],[0,255],m);
         g.drawRect(x,y,w,h);
         g.endFill();
      }
   }
}
