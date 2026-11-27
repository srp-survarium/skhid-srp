package survarium.upgrade
{
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.geom.Point;
   import flash.utils.Dictionary;
   import survarium.constants.CosmeticSlot;
   import survarium.constants.ItemCategory;
   import survarium.data.LocalizationData;

   // The modification markers over the gun at the bench, 0.69d0's workshop (its ViewWeaponWindow;
   // docs/notes/modification-069.md). The exe says each frame the gun moved where its slots are on the
   // screen (root.srp_slot_points: the gun's dict id, then a slot and its pixel a slot); the view keeps
   // the last points and shows, while the Attachments page of a gun with a ModuleTree is open, a marker
   // at each slot the tree shows: a dot on the gun, a line, a square in a row above or below the gun
   // with what stands there. A click on a square opens the slot's list (ModuleList) beside it; a choice
   // goes on the gun as the window's slots put it (what the weapon kit holds: its scope and way, muzzle
   // device, grip: the match gets those), or, for a carrier with nothing on it, is the lobby's alone
   // (`picks`, a weapon's for the session: no match sees it, and the gun is drawn without it until
   // something stands on it). The window's slots and the kit stay the one truth: the markers show what
   // they hold.
   public class ModificationView extends Sprite
   {

      // between the gun's outermost point and a row's squares
      protected static const ROW_GAP:Number = 46;

      // between two squares of a row
      protected static const BOX_GAP:Number = 12;

      // between a square and its list
      protected static const LIST_GAP:Number = 8;

      protected static const LINE:uint = 0xE7F1EE;

      // the lobby's carriers on each weapon (item id): {slot: module}
      protected static var _picks:Dictionary = new Dictionary();

      protected var _window:UpgradeWindow;

      protected var _tree:ModuleTree;

      // the weapon item the markers are of, 0 while none are shown
      protected var _host:uint = 0;

      // the last points the exe gave: their gun's dict id and each slot's pixel
      protected var _pointsGun:int = -1;

      protected var _points:Dictionary = new Dictionary();

      // the markers shown, by slot
      protected var _markers:Dictionary = new Dictionary();

      protected var _lines:Shape;

      protected var _shadows:Shape;

      protected var _list:ModuleList;

      // the marker whose list is open, or null
      protected var _open:SlotMarker = null;

      // a scope put on for a way other than its first: the way is sent once the account holds the
      // scope ({item, way}), or null
      protected var _pending:Object = null;

      public function ModificationView(window:UpgradeWindow)
      {
         super();
         this._window = window;
         mouseEnabled = false;
         this._shadows = new Shape();
         this._lines = new Shape();
         addChild(this._shadows);
         addChild(this._lines);
         this._list = new ModuleList();
         this._list.addEventListener(Event.SELECT,this.onChoose);
         addChild(this._list);
         visible = false;
      }

      // The markers of a weapon's Attachments page (`host` its item, 0 for none shown): those of its
      // tree, when it has one.
      public function showFor(host:uint, weaponDictId:int) : void
      {
         var tree:ModuleTree = host != 0 ? ModuleTree.of(weaponDictId) : null;
         if(host != this._host || tree != this._tree)
         {
            this.closeList();
            this.clearMarkers();
            this._pending = null;
         }
         this._host = Boolean(tree) ? host : 0;
         this._tree = tree;
         visible = Boolean(tree);
         this.refresh();
      }

      // root.srp_slot_points: the gun's dict id, then each slot's byte and pixel.
      public function setPoints(numbers:Array) : void
      {
         var i:int = 1;
         this._points = new Dictionary();
         this._pointsGun = numbers.length > 0 ? int(numbers[0]) : -1;
         while(i + 2 < numbers.length)
         {
            this._points[int(numbers[i])] = new Point(Number(numbers[i + 1]),Number(numbers[i + 2]));
            i += 3;
         }
         this.refresh();
      }

      // The account's update of the weapon (the window's keepHeldAttachments): a scope put on for
      // another way gets it once the account holds it (an update of before keeps it waiting; another
      // choice or another window lets it go).
      public function kitChanged() : void
      {
         var fitted:Fitted = this._window.modificationFitted();
         if(Boolean(this._pending) && Boolean(fitted.scope) && uint(fitted.scope.itemId) == uint(this._pending.item))
         {
            var way:int = int(this._pending.way);
            this._pending = null;
            this._window.chooseMount(way);
         }
         this.refresh();
      }

      // What the kit holds, a pending way taken as the scope's.
      protected function fitted() : Fitted
      {
         var fitted:Fitted = this._window.modificationFitted();
         if(Boolean(this._pending) && Boolean(fitted.scope) && uint(fitted.scope.itemId) == uint(this._pending.item))
         {
            fitted.way = int(this._pending.way);
         }
         return fitted;
      }

      protected function picks() : Object
      {
         if(!_picks[this._host])
         {
            _picks[this._host] = {};
         }
         return _picks[this._host];
      }

      // The markers again: those the tree shows with a point of this gun, each with what stands there.
      public function refresh() : void
      {
         var standing:Dictionary = null;
         var shown:Array = null;
         var wanted:Dictionary = new Dictionary();
         var row:Object = null;
         var marker:SlotMarker = null;
         var gone:Array = [];
         var fresh:int = 0;
         if(!this._tree || !visible || this._pointsGun != this._tree.weapon)
         {
            this.closeList();
            this.clearMarkers();
            return;
         }
         standing = this._tree.standing(this.fitted());
         shown = this._tree.shown(standing,this.picks());
         for each(row in shown)
         {
            if(!(row.slot in this._points))
            {
               continue;
            }
            wanted[row.slot] = row;
            marker = this._markers[row.slot] as SlotMarker;
            if(!marker)
            {
               marker = new SlotMarker(int(row.slot),LocalizationData.getName(String(row.name)));
               marker.box.addEventListener(MouseEvent.CLICK,this.onBoxClick);
               addChildAt(marker,getChildIndex(this._list));
               this._markers[row.slot] = marker;
               marker.show(fresh * 0.09);
               fresh++;
            }
            this.fill(marker,row,standing);
         }
         for each(marker in this._markers)
         {
            if(!(marker.slot in wanted))
            {
               gone.push(marker);
            }
         }
         for each(marker in gone)
         {
            this.removeMarker(marker);
         }
         if(Boolean(this._open) && !(this._open.slot in wanted))
         {
            this.closeList();
         }
         this.layout(shown,wanted);
      }

      // A marker's content: the module standing there, the carrier, or the gun's own part (or nothing);
      // made again only when it is another.
      protected function fill(marker:SlotMarker, row:Object, standing:Dictionary) : void
      {
         var here:Object = standing[row.slot];
         var carrier:Object = this._tree.carrierIn(int(row.slot),standing,this.picks());
         var content:String = Boolean(here) ? "item " + uint(here.item.itemId) : (Boolean(carrier) ? "carrier " + carrier.module : "own");
         if(content == marker.content)
         {
            return;
         }
         marker.content = content;
         if(Boolean(here))
         {
            marker.setContent(this._window.modificationIcon(uint(here.item.itemId)),String(here.item.name),true);
         }
         else if(Boolean(carrier))
         {
            marker.setContent(null,LocalizationData.getName(String(carrier.name)),true);
         }
         else
         {
            marker.setContent(null,Boolean(row.own) ? LocalizationData.getName("st_srp_modification_stock") : "",false);
         }
      }

      // The squares in their rows, along the gun as the points are, none over another; the lines.
      protected function layout(shown:Array, wanted:Dictionary) : void
      {
         var local:Dictionary = new Dictionary();
         var top:Number = Number.POSITIVE_INFINITY;
         var bottom:Number = Number.NEGATIVE_INFINITY;
         var row:Object = null;
         var at:Point = null;
         this._lines.graphics.clear();
         this._shadows.graphics.clear();
         for each(row in shown)
         {
            if(row.slot in wanted)
            {
               at = globalToLocal(this._points[row.slot] as Point);
               local[row.slot] = at;
               top = Math.min(top,at.y);
               bottom = Math.max(bottom,at.y);
            }
         }
         for each(var side:String in [ModuleTree.ABOVE,ModuleTree.BELOW])
         {
            var rowSlots:Array = shown.filter(function(item:Object, ... rest):Boolean
            {
               return item.row == side && item.slot in wanted;
            });
            var y:Number = side == ModuleTree.ABOVE ? top - ROW_GAP - SlotMarker.SIZE / 2 : bottom + ROW_GAP + SlotMarker.SIZE / 2;
            this.placeRow(rowSlots,local,y,side == ModuleTree.ABOVE);
         }
         if(Boolean(this._open))
         {
            this.placeList();
         }
      }

      // One row's squares at `y`: each under (or over) its point, pushed apart along the row's way.
      protected function placeRow(rowSlots:Array, local:Dictionary, y:Number, above:Boolean) : void
      {
         var xs:Array = [];
         var i:int = 0;
         var step:Number = SlotMarker.SIZE + BOX_GAP;
         var way:Number = 1;
         var points:Number = 0;
         var boxes:Number = 0;
         var marker:SlotMarker = null;
         var at:Point = null;
         if(rowSlots.length == 0)
         {
            return;
         }
         for each(var row:Object in rowSlots)
         {
            xs.push((local[row.slot] as Point).x);
            points += (local[row.slot] as Point).x;
         }
         way = xs[xs.length - 1] < xs[0] ? -1 : 1;
         i = 1;
         while(i < xs.length)
         {
            xs[i] = way > 0 ? Math.max(xs[i],xs[i - 1] + step) : Math.min(xs[i],xs[i - 1] - step);
            i++;
         }
         for each(var x:Number in xs)
         {
            boxes += x;
         }
         i = 0;
         while(i < rowSlots.length)
         {
            marker = this._markers[rowSlots[i].slot] as SlotMarker;
            at = local[rowSlots[i].slot] as Point;
            marker.placeDot(at.x,at.y);
            marker.placeBox(xs[i] + (points - boxes) / xs.length,y,above);
            this.drawLine(marker,at);
            i++;
         }
      }

      // The line from the dot's edge to the square's (0.69d0's: 1.4 px, its shadow blurred under it).
      protected function drawLine(marker:SlotMarker, at:Point) : void
      {
         var dx:Number = marker.box.x - at.x;
         var dy:Number = marker.box.y - at.y;
         var length:Number = Math.sqrt(dx * dx + dy * dy);
         var half:Number = SlotMarker.SIZE / 2;
         var into:Number = Math.max(Math.abs(dx),Math.abs(dy));
         if(length < 8 || into <= half)
         {
            return;
         }
         var from:Point = new Point(at.x + dx * 6 / length,at.y + dy * 6 / length);
         var to:Point = new Point(marker.box.x - dx * half / into,marker.box.y - dy * half / into);
         this._shadows.graphics.lineStyle(4,0,0.13);
         this._shadows.graphics.moveTo(from.x,from.y);
         this._shadows.graphics.lineTo(to.x,to.y);
         this._lines.graphics.lineStyle(1.4,LINE,0.65);
         this._lines.graphics.moveTo(from.x,from.y);
         this._lines.graphics.lineTo(to.x,to.y);
      }

      protected function onBoxClick(event:MouseEvent) : void
      {
         var marker:SlotMarker = (event.currentTarget as Sprite).parent as SlotMarker;
         if(!marker)
         {
            return;
         }
         if(marker == this._open)
         {
            this.closeList();
            return;
         }
         this.openList(marker);
      }

      // A slot's list: what the gun has there of its own (or nothing), the carriers that go there, the
      // inventory's modules that go there; what stands there marked.
      protected function openList(marker:SlotMarker) : void
      {
         var row:Object = this._tree.slot(marker.slot);
         var standing:Dictionary = this._tree.standing(this.fitted());
         var here:Object = standing[row.slot];
         var carrier:Object = this._tree.carrierIn(int(row.slot),standing,this.picks());
         var entries:Array = [];
         var kit:int = ModuleTree.kitSlot(String(row.takes));
         var weapon:int = this._tree.weapon;
         var tree:ModuleTree = this._tree;
         entries.push({
            "label":LocalizationData.getName(Boolean(row.own) ? "st_srp_modification_stock" : "st_srp_modification_empty"),
            "icon":null,
            "standing":!here && !carrier,
            "carrier":null,
            "item":null
         });
         for each(var each:Object in this._tree.carriers)
         {
            if(each.slot == row.slot)
            {
               entries.push({
                  "label":LocalizationData.getName(String(each.name)),
                  "icon":null,
                  "standing":carrier == each,
                  "carrier":each,
                  "item":null
               });
            }
         }
         if(row.takes != ModuleTree.PLAIN)
         {
            for each(var item:Object in this._window.ownedFitting(kit,function(fit:Object):Boolean
            {
               return kit != CosmeticSlot.CAMOUFLAGE || int(fit.category) == ItemCategory.SCOPES && tree.waysOf(int(fit.dictId)).indexOf(int(row.slot)) != -1;
            }))
            {
               entries.push({
                  "label":String(item.name),
                  "icon":this._window.modificationIcon(uint(item.itemId)),
                  "standing":Boolean(here) && uint(here.item.itemId) == uint(item.itemId),
                  "carrier":null,
                  "item":item
               });
            }
         }
         if(Boolean(this._open))
         {
            this._open.open = false;
         }
         this._open = marker;
         marker.open = true;
         this._list.setEntries(LocalizationData.getName(String(row.name)),entries);
         this._list.visible = true;
         this.placeList();
      }

      // The list beside its square: to its right, or to its left at the stage's right edge; within the
      // stage's height.
      protected function placeList() : void
      {
         var box:Sprite = this._open.box;
         var right:Point = localToGlobal(new Point(box.x + SlotMarker.SIZE / 2 + LIST_GAP + ModuleList.WIDTH,box.y));
         var x:Number = box.x + SlotMarker.SIZE / 2 + LIST_GAP;
         var y:Number = box.y - SlotMarker.SIZE / 2;
         var bottom:Point = null;
         if(Boolean(stage) && right.x > stage.stageWidth)
         {
            x = box.x - SlotMarker.SIZE / 2 - LIST_GAP - ModuleList.WIDTH;
         }
         bottom = localToGlobal(new Point(x,y + this._list.height));
         if(Boolean(stage) && bottom.y > stage.stageHeight)
         {
            y -= globalToLocal(bottom).y - globalToLocal(new Point(0,stage.stageHeight)).y;
         }
         this._list.x = Math.round(x);
         this._list.y = Math.round(y);
      }

      protected function closeList() : void
      {
         if(Boolean(this._open))
         {
            this._open.open = false;
         }
         this._open = null;
         this._list.clear();
         this._list.visible = false;
      }

      // A row of the list chosen: it goes on the gun.
      protected function onChoose(event:Event) : void
      {
         var entry:Object = this._list.chosen;
         var row:Object = Boolean(this._open) ? this._tree.slot(this._open.slot) : null;
         if(!entry || !row)
         {
            return;
         }
         this.closeList();
         this.choose(row,entry);
         this.refresh();
      }

      // What a choice does in `row`'s slot. Nothing: what stands there comes off, and a carrier there
      // with everything on it. A carrier: what stands there comes off (and another carrier there with
      // what is on it); it is the lobby's pick until something stands on it. A module: it goes on as the
      // window puts it (the kit's), a scope on the way to this slot (its way sent once the account holds
      // it); a pick in this slot is let go.
      protected function choose(row:Object, entry:Object) : void
      {
         var standing:Dictionary = this._tree.standing(this.fitted());
         var here:Object = standing[row.slot];
         var carrier:Object = this._tree.carrierIn(int(row.slot),standing,this.picks());
         var fitted:Fitted = this._window.modificationFitted();
         var item:Object = entry.item;
         var way:int = 0;
         this._pending = null;
         if(Boolean(item))
         {
            delete this.picks()[row.slot];
            if(row.takes != ModuleTree.SCOPE)
            {
               this._window.fitModule(ModuleTree.kitSlot(String(row.takes)),uint(item.itemId));
               return;
            }
            way = this._tree.waysOf(int(item.dictId)).indexOf(int(row.slot));
            if(Boolean(fitted.scope) && uint(fitted.scope.itemId) == uint(item.itemId))
            {
               this._window.chooseMount(way);
               return;
            }
            this._pending = way != 0 ? {"item":uint(item.itemId),"way":way} : null;
            this._window.fitModule(CosmeticSlot.CAMOUFLAGE,uint(item.itemId));
            return;
         }
         if(Boolean(here))
         {
            this._window.takeOff(ModuleTree.kitSlot(String(here.takes)));
         }
         if(Boolean(carrier) && carrier != entry.carrier)
         {
            this.takeOffFrom(carrier,standing);
         }
         if(Boolean(entry.carrier))
         {
            this.picks()[row.slot] = String(entry.carrier.module);
         }
         else
         {
            delete this.picks()[row.slot];
         }
      }

      // Everything standing in the slots a carrier brings comes off.
      protected function takeOffFrom(carrier:Object, standing:Dictionary) : void
      {
         for each(var brought:int in carrier.brings)
         {
            if(Boolean(standing[brought]))
            {
               this._window.takeOff(ModuleTree.kitSlot(String(standing[brought].takes)));
            }
         }
      }

      protected function removeMarker(marker:SlotMarker) : void
      {
         marker.box.removeEventListener(MouseEvent.CLICK,this.onBoxClick);
         marker.destroy();
         if(marker.parent == this)
         {
            removeChild(marker);
         }
         delete this._markers[marker.slot];
      }

      protected function clearMarkers() : void
      {
         for each(var marker:SlotMarker in this._markers)
         {
            marker.box.removeEventListener(MouseEvent.CLICK,this.onBoxClick);
            marker.destroy();
            if(marker.parent == this)
            {
               removeChild(marker);
            }
         }
         this._markers = new Dictionary();
         this._lines.graphics.clear();
         this._shadows.graphics.clear();
      }
   }
}
