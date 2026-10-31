package survarium.controls
{
   import flash.display.MovieClip;
   import flash.utils.Dictionary;
   import scaleform.clik.controls.TileList;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.layout.LayoutData;
   import survarium.core.Icon;
   import survarium.data.IconContainer;
   import survarium.data.ItemDescriptor;
   
   public class QuickSlots extends MovieClip
   {
      
      public var layoutData:LayoutData;
      
      public var slots_list:TileList;
      
      private var _itemDescriptor:ItemDescriptor;
      
      private var _iconContainer:IconContainer;
      
      private var _inventoryItems:Array;
      
      private var _slots_items:Array = new Array();

      // This owner combines native inventory descriptions with native effect
      // snapshots. Neither callback can erase the other's presentation state.
      // Phases: 0 none, 1 waiting (the use time), 2 active, 3 stopped by a
      // bullet (patch 84; the hook sends it for a second of game time).
      public static const ACTIVITY_INTERRUPTED:uint = 3;
      private var _nativeSlots:Array = [];
      private var _activities:Array = [];

      public function setActivity(slot:uint, phase:uint, remaining:uint, duration:uint) : Boolean
      {
         if(slot >= 6) return false;
         var seconds:uint = phase == 2 ? Math.ceil(remaining / 1000) : 0;
         var activation:uint = phase == 1 && duration > 0 ? Math.min(100,Math.ceil(100 * remaining / duration)) : 0;
         var old:Object = _activities[slot];
         if(old && old.phase == phase && old.seconds == seconds && old.activation == activation) return false;
         _activities[slot] = {phase:phase, seconds:seconds, activation:activation};
         return _nativeSlots[slot] ? renderSlot(presentation(_nativeSlots[slot])) : false;
      }

      private function presentation(nativeSlot:Object) : Object
      {
         var result:Object = {};
         for(var key:String in nativeSlot) result[key] = nativeSlot[key];
         var activity:Object = _activities[uint(nativeSlot.slot_id)];
         // An exhausted stack still owns its ongoing native effect. Keep its
         // gray icon and native radial sweep during activation, then seconds only;
         // a stopped one keeps its icon, red, while the hook says so.
         if(activity && activity.phase != 0 && !nativeSlot.is_artefact)
         {
            result.enabled = true;
            result.activityPhase = activity.phase;
            result.activitySeconds = activity.seconds;
            result.cooldown = activity.phase == 1 ? Math.max(1,activity.activation) : 0;
         }
         else
         {
            result.activityPhase = 0;
            result.activitySeconds = 0;
         }
         return result;
      }
      
      protected var _grayBitmaps:Dictionary = new Dictionary();
      
      protected var otherRows:uint = 22;
      
      // An item's quick-slot icon is the cell of hudItems whose index its icon id (ui_desc/icon:
      // (column - 1) x 22 + row - 1) has in hudIcons; an id not there shows none. 0.26g's items fill
      // cells 0 to 29 (6 to 9 its oxygen tanks, 28 and 29 the exoskeletons' backs). The id does not
      // say which lobby atlas the cell is of (the drugs' consumables, the tanks' backs): an item's
      // cell must give an id no other quick-slot item's gives.
      protected static const NATIVE_HUD_ICONS:Array = [17,18,19,20,21,22,13,42,67,74,14,15,9,23,12,49,50,16,10,11,51,52,53,-1,-1,-1,-1,-1,110,113];

      // srp's items with a quick slot (its packs' oxygen tanks and drugs): their icon id and their
      // bitmap of bitmaps/, drawn in hudItems' free cells from SRP_FIRST_CELL, in this order.
      public static const SRP_HUD_ICONS:Array = [
         {icon:302, bitmap:HudItemDoubleCylinder},     // 931 Double cylinder (equipment-029), icon 17, 14
         {icon:59, bitmap:HudItemMilitaryMedkit},      // 390 Military Medkit (medicines), icon 16, 3
         {icon:60, bitmap:HudItemBlackMarketMedkit},   // 1196 Black Market Medkit (medicines), icon 17, 3
         {icon:61, bitmap:HudItemScienceMedkit}        // 391 Science Medkit (medicines), icon 18, 3
      ];

      public static const SRP_FIRST_CELL:uint = NATIVE_HUD_ICONS.length;

      protected const hudIcons:Array = NATIVE_HUD_ICONS.concat(SRP_HUD_ICONS.map(function(entry:Object, index:int, all:Array):int
      {
         return entry.icon;
      }));

      public function QuickSlots()
      {
         super();
      }

      public function registerIconContainer(link:IconContainer) : void
      {
         _iconContainer = link;
         for(var i:uint = 0; i < SRP_HUD_ICONS.length; i++)
         {
            var bitmap:Class = SRP_HUD_ICONS[i].bitmap;
            link.setIcon(SRP_FIRST_CELL + i,new bitmap());
         }
      }
      
      public function setGrayIcons(value:Dictionary) : void
      {
         var str:*;
         for(str in value)
         {
            trace("grayIndexs::",str);
         }
         _grayBitmaps = value;
      }
      
      public function fillSlot(_p:Object) : Boolean
      {
         _nativeSlots[uint(_p.slot_id)] = _p;
         return renderSlot(presentation(_p));
      }

      private function renderSlot(_p:Object) : Boolean
      {
         var convertedObject:Object;
         trace("fill_slot::",_p.cooldown,_p.name,_p.icon,_p.condition_or_stack,_p.hotkey,_p.count,"slot::",_p.slot_id,_p.timer,_p.enabled,_p.is_artefact);
         var len:uint = this._slots_items.length;
         var found:Boolean = false;
         for(var i:uint = 0; i < len; i++)
         {
            var origin:Object;
            var data:Object;
            if(this._slots_items[i].slot_id == _p.slot_id)
            {
               found = true;
               if(!_p.enabled)
               {
                  this._slots_items.splice(i,1);
               }
               else
               {
                  origin = this._slots_items[i];
                  if(_p.icon == origin.icon_id && _p.count == origin.count && _p.is_artefact == origin.is_artefact && _p.timer == origin.timer && _p.cooldown == origin.cooldown && _p.enabled == origin.enabled && _p.hotkey == origin.hotkey && _p.activityPhase == origin.activityPhase && _p.activitySeconds == origin.activitySeconds)
                  {
                     return false;
                  }
                  this._slots_items[i] = this.create_object(_p,origin);
               }
               break;
            }
         }
         if(!found)
         {
            if(_p.enabled)
            {
               data = this.create_object(_p,null);
               for(i = 0; i <= len; i++)
               {
                  trace("i_len",i,len);
                  if(i == len)
                  {
                     this._slots_items.push(data);
                     break;
                  }
                  else if(this._slots_items[i].slot_id > _p.slot_id)
                  {
                     this._slots_items.splice(i,0,data);
                     break;
                  }
               }
            }
            else
            {
               return false;
            }
         }
         trace("_slots_items::",this._slots_items,visible,alpha);
         this.slots_list.visible = this._slots_items.length > 0;
         this.slots_list.dataProvider = new DataProvider(this._slots_items);
         this.slots_list.width = this._slots_items.length ? this._slots_items.length * this.slots_list.columnWidth + 8 : 10;
         return this._slots_items.length != len;
      }
      
      public function clean() : void
      {
         trace("QS::clean");
         this._slots_items.length = 0;
         _nativeSlots = [];
         _activities = [];
         this.slots_list.dataProvider = new DataProvider([]);
         this.slots_list.width = 10;
         this.slots_list.visible = false;
      }
      
      public function fillSlots(p:Array) : void
      {
         trace("QS::fillSlots");
         _nativeSlots = [];
         _activities = [];
         this._slots_items.length = 0;
         var convertedItems:Array = new Array();
         for(var i:uint = 0; i < p.length; i++)
         {
            var currentItemDescr:Object;
            var iconRow:int;
            var iconColumn:int;
            var iconId:int;
            var iconIndex:int;
            var icon:Icon;
            var grayIcon:Icon;
            currentItemDescr = p[i];
            _nativeSlots[uint(currentItemDescr.slot_id)] = currentItemDescr;
            if(currentItemDescr.icon_row == undefined)
            {
               currentItemDescr.icon_row = currentItemDescr.icon % this.otherRows + 1;
               currentItemDescr.icon_column = int(currentItemDescr.icon / this.otherRows) + 1;
               trace("hud_create_icon",currentItemDescr.icon,"row_column::",currentItemDescr.icon_row,currentItemDescr.icon_column,"rows::",this.otherRows);
            }
            iconRow = currentItemDescr.icon_row - 1;
            iconColumn = currentItemDescr.icon_column - 1;
            iconId = iconColumn * this.otherRows + iconRow;
            currentItemDescr.icon = iconId;
            iconIndex = this.hudIcons.indexOf(currentItemDescr.icon);
            icon = this._iconContainer.getIcon(iconIndex);
            trace("----Slots:createIcon::","rowcolumn",iconRow,iconColumn,"index::",iconIndex,"id:",iconId,icon,this._grayBitmaps[iconIndex]);
            grayIcon = null;
            if(this._grayBitmaps[iconIndex])
            {
               grayIcon = (this._grayBitmaps[iconIndex] as Icon).clone();
            }
            if(currentItemDescr.enabled)
            {
               convertedItems.push({
                  id:currentItemDescr.id,
                  slot_id:currentItemDescr.slot_id,
                  dictId:currentItemDescr.dictId,
                  name:currentItemDescr.name,
                  icon:icon,
                  grayIcon:grayIcon,
                  icon_id:currentItemDescr.icon,
                  count:currentItemDescr.count,
                  cooldown:currentItemDescr.cooldown,
                  enabled:currentItemDescr.enabled,
                  hotkey:currentItemDescr.hotkey,
                  timer:currentItemDescr.timer,
                  is_artefact:currentItemDescr.is_artefact
               });
            }
         }
         _slots_items = convertedItems;
         this.slots_list.width = this._slots_items.length ? this._slots_items.length * this.slots_list.columnWidth + 8 : 10;
         this.slots_list.visible = this._slots_items.length > 0;
         this.slots_list.dataProvider = new DataProvider(convertedItems);
         trace("vs::",visible,parent,width,this.slots_list.width,this.slots_list.visible);
         parent["layout"].reflow();
      }
      
      protected function create_object(value:Object, current_value:Object) : Object
      {
         var iconRow:int = value.icon_row - 1;
         var iconColumn:int = value.icon_column - 1;
         var iconId:int = iconColumn * this.otherRows + iconRow;
         value.icon = iconId;
         var isSame:Boolean = current_value && value.icon == current_value.icon_id;
         var iconIndex:int = this.hudIcons.indexOf(value.icon);
         var icon:Icon = isSame ? current_value.icon : this._iconContainer.getIcon(iconIndex);
         var grayIcon:Icon = null;
         if(this._grayBitmaps[iconIndex])
         {
            grayIcon = isSame ? current_value.grayIcon : (this._grayBitmaps[iconIndex] as Icon).clone();
         }
         return {
            id:value.id,
            slot_id:value.slot_id,
            icon:icon,
            grayIcon:grayIcon,
            icon_id:value.icon,
            count:value.count,
            cooldown:value.cooldown,
            enabled:value.enabled,
            hotkey:value.hotkey,
            timer:value.timer,
            activityPhase:value.activityPhase,
            activitySeconds:value.activitySeconds,
            is_artefact:value.is_artefact
         };
      }
   }
}

