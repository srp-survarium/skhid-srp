package survarium.paperdoll
{
   import flash.events.Event;
   import flash.events.MouseEvent;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.DragSlot;
   import scaleform.clik.controls.ToolTip;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.interfaces.IDataProvider;
   import scaleform.clik.interfaces.IListItemRenderer;
   import survarium.constants.Category;
   import survarium.data.ItemData;
   import survarium.data.LocalizationData;
   import survarium.interfaces.IPaperDoll;
   import survarium.interfaces.IPaperDollSlot;
   
   public class PaperDollBase extends UIComponent implements IPaperDoll
   {
      
      public static const PAPERDOLL_UPDATED:String = "paperDollUpdated";
      
      protected static var _slotNames:Array = ["slotHead","slotMask","slotBody","slotBack","slotLegs","slotArms","slotFoot","slotWpn1","slotWpn1A1","slotWpn1A2","slotWpn2","slotWpn2A1","slotWpn2A2","slotQck1","slotQck2","slotQck3","slotQck4","slotQck5","slotQck6","slotWpn5","slotWpn6","slotWpn7","slotWpn8"];
      
      protected var _slots:Array = [];
      
      protected var _dataProvider:IDataProvider;
      
      protected var _slotTypes:Array = ["head","mask","torso","back","legs","hands","foot","weapon","ammo","ammo","weapon","ammo","ammo","slot","slot","slot","slot","slot","slot"];
      
      protected var _slotLabels:Array = ["filter_helmet","filter_mask","filter_body_armor","filter_rig","filter_pants","filter_gloves","filter_boots","pd_weapon","pd_ammo","pd_ammo","pd_weapon","pd_ammo","pd_ammo","pd_artifact","pd_artifact","pd_artifact","pd_artifact","pd_artifact","pd_artifact","pd_ammo","pd_ammo","pd_ammo","pd_ammo"];
      
      protected var _slotsN:int = 0;
      
      protected var _slotsA:int = 0;
      
      public function PaperDollBase()
      {
         super();
      }
      
      override protected function initialize() : void
      {
         super.initialize();
         _slots = [];
         tabEnabled = tabChildren = false;
      }
      
      public function get dataProvider() : IDataProvider
      {
         return this._dataProvider;
      }
      
      public function set dataProvider(value:IDataProvider) : void
      {
         if(this._dataProvider == value)
         {
            return;
         }
         if(this._dataProvider != null)
         {
            this._dataProvider.removeEventListener(Event.CHANGE,this.handleDataChange,false);
         }
         _dataProvider = value;
         if(this._dataProvider == null)
         {
            return;
         }
         this._dataProvider.addEventListener(Event.CHANGE,this.handleDataChange,false,0,true);
         invalidateData();
      }
      
      public function getSlotAt(value:int) : IPaperDollSlot
      {
         return this._slots[value];
      }
      
      public function getSlotByName(value:String) : int
      {
         var index:int = _slotNames.indexOf(value);
         if(index == -1)
         {
            index = 1;
         }
         return this._dataProvider[index];
      }
      
      override public function toString() : String
      {
         return "[PaperDollBase " + name + "]";
      }
      
      public function openSlots(slotsN:int, slotsA:int) : void
      {
         _slotsN = slotsN;
         _slotsA = slotsA;
         invalidateData();
      }
      
      protected function handleDataChange(event:Event) : void
      {
         invalidate(InvalidationType.DATA);
      }
      
      protected function updateSlotLabels(value:String) : void
      {
         var i:uint = 0;
         var slot:IPaperDollSlot;
         for(; i < _slotNames.length; i++)
         {
            slot = getChildByName(_slotNames[i]) as IPaperDollSlot;
            if(!slot)
            {
               continue;
            }
            slot.label = LocalizationData.getName(this._slotLabels[i]);
         }
      }
      
      protected function configSlot(slot:IPaperDollSlot) : void
      {
         (slot as DragSlot).dragEnabled = false;
         (slot as ToolTip).tooltipRendererName = "SrvItemHintRednerer";
      }
      
      protected function handleSlotListeners(slot:IPaperDollSlot) : void
      {
         slot.addEventListener(MouseEvent.ROLL_OVER,this.newItemOver);
         slot.addEventListener(MouseEvent.ROLL_OUT,this.newItemOut);
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         LocalizationData.addCallback(this.updateSlotLabels,"sell");
         var i:uint = 0;
         var slot:IPaperDollSlot;
         for(; i < _slotNames.length; i++)
         {
            slot = getChildByName(_slotNames[i]) as IPaperDollSlot;
            if(!slot)
            {
               continue;
            }
            slot.index = i;
            slot.owner = this;
            slot.selected = false;
            slot.label = LocalizationData.getName(this._slotLabels[i]);
            this._slots[i] = slot;
            this.handleSlotListeners(slot);
            this.configSlot(slot);
         }
      }
      
      protected function sortAmmo() : void
      {
      }
      
      override protected function draw() : void
      {
         var newslots:Array;
         var newitems:IDataProvider;
         var newItemsL:uint;
         var i:uint;
         var slotsL:uint;
         var j:uint;
         var newSlotData:ItemData;
         var slot:IPaperDollSlot;
         var slotData:ItemData;
         var isAmmoSlot:Boolean;
         super.draw();
         if(isInvalid(InvalidationType.DATA))
         {
            newslots = new Array();
            this.sortAmmo();
            newitems = this._dataProvider;
            if(this._dataProvider != null)
            {
               newItemsL = newitems.length;
               i = 0;
               while(i < newItemsL)
               {
                  slotData = newitems[i];
                  if(slotData.additionalSlots)
                  {
                     j = 0;
                     while(j < slotData.additionalSlots.length)
                     {
                        newSlotData = slotData.clone();
                        newSlotData.additionalSlots = null;
                        newSlotData.enabled = false;
                        newSlotData.slotId = slotData.additionalSlots[j];
                        newslots[newSlotData.slotId] = newSlotData;
                        j++;
                     }
                  }
                  i++;
               }
               i = 0;
               while(i < newItemsL)
               {
                  trace("newItemsL",i,newitems[i].name,newitems[i].slotId);
                  newslots[newitems[i].slotId] = newitems[i];
                  i++;
               }
               slotsL = this._slots.length;
               i = 0;
               for(; i < slotsL; i++)
               {
                  slot = this._slots[i] as IPaperDollSlot;
                  trace("DRAW_PDB",i,slot,newslots[i]);
                  if(!slot)
                  {
                     continue;
                  }
                  slotData = newslots[i];
                  isAmmoSlot = slotData && slotData.superCategory == Category.BASE_AMMO;
                  if(slotData && (isAmmoSlot && slotData.value != 0 || !isAmmoSlot))
                  {
                     slot.setData(slotData);
                     slot.enabled = slotData.enabled;
                  }
                  else
                  {
                     slot.setData(null);
                     slot.enabled = true;
                  }
                  slot.invalidateData();
                  slot.validateNow();
               }
            }
            this.handleQuickSlots();
            dispatchEvent(new Event(PAPERDOLL_UPDATED));
         }
      }
      
      protected function handleQuickSlots() : void
      {
         var slots:int = this._slotsN + this._slotsA;
         var i:uint = 1;
         var slotRenderer:IPaperDollSlot;
         while(i < 7)
         {
            slotRenderer = this._slots[i + 12];
            trace("handleQuickSlots",slotRenderer);
            if(i > slots)
            {
               if(slotRenderer.data)
               {
                  slotRenderer.dropSlot();
               }
               slotRenderer.enabled = false;
               slotRenderer.artifact = false;
               slotRenderer.visible = false;
            }
            else
            {
               slotRenderer.visible = true;
               if(i > this._slotsN)
               {
                  slotRenderer.label = LocalizationData.getName("pd_artifact");
                  slotRenderer.artifact = true;
                  if(slotRenderer.data)
                  {
                     slotRenderer.dropSlot();
                  }
                  slotRenderer.enabled = false;
               }
               else
               {
                  slotRenderer.label = LocalizationData.getName("filter_items");
                  slotRenderer.artifact = false;
                  slotRenderer.enabled = true;
               }
            }
            i++;
         }
      }
      
      protected function newItemOut(e:MouseEvent) : void
      {
         if(!e || !e.target)
         {
            return;
         }
         var btn:* = e.target;
         dispatchEvent(new ListEvent(ListEvent.ITEM_ROLL_OUT,true,false,e.target.index,-1,-1,e.target as IListItemRenderer,e.target.data));
      }
      
      protected function newItemOver(e:MouseEvent) : void
      {
         if(!e || !e.target)
         {
            return;
         }
         var btn:* = e.target;
         dispatchEvent(new ListEvent(ListEvent.ITEM_ROLL_OVER,true,false,e.target.index,-1,-1,e.target as IListItemRenderer,e.target.data));
      }
   }
}

