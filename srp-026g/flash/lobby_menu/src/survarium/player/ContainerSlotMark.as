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
   import survarium.constants.UiSound;
   import survarium.data.LocalizationData;
   import survarium.rooms.RoomsLook;

   // Artifact Container's slot (artefact-skills.md, "Artifact Container (522)"): with the skill,
   // the quick slot it turns into an artefact container is marked with a soft edge of the lobby's
   // yellow, as the container or as the pocket it was, and a click switches it (srp's word, the
   // presets list's `K` row, says which it is; LocalPlayerProfile.containerSlot). The one owner of
   // that state: the paper doll places it (LocalPaperDoll.handleQuickSlots) and hands it the
   // slot's clicks.
   //
   // - As the container (a locked slot takes no mouse): the mark takes the hover (the hint) and
   //   the click itself.
   // - As the pocket: the slot keeps the mouse (drops, its item); the paper doll forwards its
   //   hover and its click. A click on it while it holds an item is the item's, as any pocket's:
   //   it is made a container only empty (the owner's pick of 2026-11-22), so a stray click
   //   never moves an item.
   // - A click shows the other look at once (a cross-fade) and asks srp; the slot stays where it
   //   was until srp's answer (the account, then the `K` row) says so too, so nothing jumps.
   public class ContainerSlotMark extends Sprite
   {

      // The kit's slot as srp says it (ContainerSlot::code; 0: no such slot, no skill).
      public static const NONE:int = 0;

      public static const CONTAINER:int = 1;

      public static const POCKET:int = 2;

      // The cross-fade between the two states' looks, and the hover's.
      private static const FADE_MS:Number = 130;

      // A click's answer is the account and the list with the `K` row; none in this long: srp's
      // last word is shown again.
      private static const PENDING_MS:int = 3000;

      // srp's word, the look shown (the click's while it is asked), and since when it is asked.
      private var _state:int = NONE;

      private var _shown:int = NONE;

      private var _pendingAt:int = -1;

      // The kit the state is of (a kit shown anew starts from srp's word).
      private var _kit:uint = 0;

      // The quick slot it is on (slotIndex), kept while a click is asked.
      private var _index:int = -1;

      // The slot it is on, null: none, its size drawn, and whether it holds an item.
      private var _slot:DisplayObject;

      private var _w:Number = 0;

      private var _h:Number = 0;

      private var _holds:Boolean = false;

      // Each state's look at rest and what the hover adds ([NONE] unused), and how much of each is
      // shown now, eased toward its target a frame at a time.
      private var _rest:Array = [null,new Shape(),new Shape()];

      private var _lift:Array = [null,new Shape(),new Shape()];

      private var _show:Array = [0,0,0];

      private var _over:Number = 0;

      private var _overWanted:Boolean = false;

      private var _frameAt:int = -1;

      // The hint, the game's tooltip; its area the hover and click area as the container.
      private var _tip:ReasonsTip;

      // ask(code): the server asked to make the kit's slot that one (LocalPlayerProfile).
      private var _ask:Function;

      public function ContainerSlotMark()
      {
         super();
         visible = false;
         for each(var state:int in [CONTAINER,POCKET])
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

      // The look shown (the click's while it is asked).
      public function get shown() : int
      {
         return this._shown;
      }

      // srp's word for the kit `kit` shown, and how to ask it for another. While a click is asked,
      // a word that is not its answer changes nothing shown (an older list on its way).
      public function told(kit:uint, state:int, ask:Function) : void
      {
         this._state = state == CONTAINER || state == POCKET ? state : NONE;
         this._ask = ask;
         if(kit != this._kit)
         {
            this._kit = kit;
            this._pendingAt = -1;
            this.show(this._state,true);
            return;
         }
         if(this.pending() && this._state != this._shown)
         {
            return;
         }
         this._pendingAt = -1;
         this.show(this._state,false);
      }

      // The quick slot (13 + i) the skill's slot is among `pockets` pockets and `containers`
      // containers after them (PaperDollBase's counts): the first container as the container,
      // the last pocket as the pocket; -1: none shown. Kept while a click is asked: the counts
      // come before the `K` row.
      public function slotIndex(pockets:int, containers:int) : int
      {
         if(this.pending())
         {
            return this._index;
         }
         this._index = -1;
         if(this._shown == CONTAINER && containers >= 1)
         {
            this._index = pockets;
         }
         else if(this._shown == POCKET && pockets >= 1)
         {
            this._index = pockets - 1;
         }
         return this._index;
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
         if(w != this._w || h != this._h)
         {
            this._w = w;
            this._h = h;
            this.drawLooks();
         }
         this._tip.anchor = slot;
         this._tip.tipData = {"name":this.hint()};
         // As the container the mark takes the mouse; as the pocket the slot keeps it.
         mouseEnabled = mouseChildren = this._shown == CONTAINER;
         visible = true;
      }

      // The hover: the look lifted; as the pocket (the paper doll's word, the slot keeps the
      // mouse) the hint too, while it is empty.
      public function hover(over:Boolean) : void
      {
         this._overWanted = over;
         this.fade();
         if(over && this._shown == POCKET && !this._holds && visible)
         {
            this._tip.tipData = {"name":this.hint()};
            this._tip.showNow();
         }
         else if(!over || this._shown != CONTAINER)
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

      private function onOver(e:MouseEvent) : void
      {
         this.hover(true);
      }

      private function onOut(e:MouseEvent) : void
      {
         this.hover(false);
      }

      private function pending() : Boolean
      {
         return this._pendingAt >= 0 && getTimer() - this._pendingAt < PENDING_MS;
      }

      // What a click asks: the container a pocket, an empty pocket the container again; an item
      // in the pocket keeps it as it is. Shown at once; one ask at a time.
      private function clicked() : void
      {
         // The paper doll greyed (the kits locked: in a match, on the way to one): no change.
         var doll:UIComponent = parent as UIComponent;
         if(!visible || this._ask == null || doll != null && !doll.enabled || this.pending())
         {
            return;
         }
         var wanted:int = NONE;
         if(this._shown == CONTAINER)
         {
            wanted = POCKET;
         }
         else if(this._shown == POCKET && !this._holds)
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
         this.show(wanted,false);
         mouseEnabled = mouseChildren = wanted == CONTAINER;
         this._tip.tipData = {"name":this.hint()};
         this._ask(wanted);
      }

      // `state`'s look shown: at once, or cross-faded from the one before.
      private function show(state:int, now:Boolean) : void
      {
         this._shown = state;
         if(now)
         {
            for each(var s:int in [CONTAINER,POCKET])
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
         for each(var s:int in [CONTAINER,POCKET])
         {
            var target:Number = s == this._shown ? 1 : 0;
            this._show[s] = toward(this._show[s],target,step);
            settled = settled && this._show[s] == target;
         }
         var over:Number = this._overWanted ? 1 : 0;
         this._over = toward(this._over,over,step);
         settled = settled && this._over == over;
         this.apply();
         // A click asked and never answered: srp's last word again.
         if(this._pendingAt >= 0 && !this.pending())
         {
            this._pendingAt = -1;
            this._shown = this._state;
            settled = false;
         }
         if(settled && this._pendingAt < 0)
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
         for each(var s:int in [CONTAINER,POCKET])
         {
            this._rest[s].alpha = ease(this._show[s]);
            this._lift[s].alpha = ease(this._show[s]) * ease(this._over);
         }
      }

      // Each state's look at rest and its hover's lift, for the slot's size: the soft edge (the
      // owner's pick of 2026-11-22), a 1 px line at half strength and the yellow glowing into the
      // slot from it, all round as the container, from the bottom up as the pocket. The hover
      // brings the line to the full yellow and the glow up. The glow is drawn as gradients: a
      // GlowFilter would have Scaleform render the mark into a bitmap of its own at every change
      // of its alpha (each frame of a fade); four gradients are four quads.
      private function drawLooks() : void
      {
         var w:Number = this._w;
         var h:Number = this._h;
         var edge:Array = [[0,0,w,1],[0,h - 1,w,1],[0,1,1,h - 2],[w - 1,1,1,h - 2]];
         var depth:Number = Math.round(Math.min(w,h) / 9);
         var rise:Number = Math.round(h / 2);
         for each(var s:int in [CONTAINER,POCKET])
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
               if(s == CONTAINER)
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

      // What a click would make it.
      private function hint() : String
      {
         return LocalizationData.getName(this._shown == CONTAINER ? "st_srp_loadout_slot_to_pocket" : "st_srp_loadout_slot_to_container");
      }
   }
}
