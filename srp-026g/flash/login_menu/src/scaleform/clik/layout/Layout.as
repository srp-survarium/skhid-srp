package scaleform.clik.layout
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.display.StageAlign;
   import flash.events.Event;
   import flash.geom.Rectangle;
   import scaleform.clik.constants.LayoutMode;
   import scaleform.clik.events.ResizeEvent;
   import scaleform.gfx.Extensions;
   
   public class Layout extends Sprite
   {
      
      public static const STAGE_ALIGN_CENTER:String = "";
      
      public var identifier:String = null;
      
      public var afterReflow:Function = null;
      
      public var beforeReflow:Function = null;
      
      protected var _rect:Rectangle = null;
      
      protected var _tiedToStageSize:Boolean = false;
      
      protected var _tiedToParent:Boolean = false;
      
      protected var _hidden:Boolean = false;
      
      protected var _parent:Sprite = null;
      
      protected var _managedSprites:Vector.<Sprite>;
      
      protected var _aspectRatio:String = "";
      
      protected var _scaleFactor:Number = 1;
      
      public function Layout()
      {
         super();
         this.initialize();
      }
      
      public function initialize() : void
      {
         addEventListener(Event.ADDED_TO_STAGE,this.handleAddedToStage,false,0,true);
         _managedSprites = new Vector.<Sprite>();
      }
      
      public function get rect() : Rectangle
      {
         return this._rect;
      }
      
      public function set rect(value:Rectangle) : void
      {
         _rect = value;
      }
      
      [Inspectable(defaultValue="false")]
      public function get tiedToStageSize() : Boolean
      {
         return this._tiedToStageSize;
      }
      
      public function set tiedToStageSize(value:Boolean) : void
      {
         if(value == this._tiedToStageSize)
         {
            return;
         }
         _tiedToStageSize = value;
         _tiedToParent = false;
         if(stage != null)
         {
            stage.removeEventListener(Event.RESIZE,this.handleStageResize,false);
            if(this._tiedToStageSize)
            {
               stage.addEventListener(Event.RESIZE,this.handleStageResize,false,0,true);
               this.updateAspectRatio();
            }
         }
      }
      
      [Inspectable(defaultValue="false")]
      public function get tiedToParent() : Boolean
      {
         return this._tiedToParent;
      }
      
      public function set tiedToParent(value:Boolean) : void
      {
         if(value == this._tiedToParent)
         {
            return;
         }
         _tiedToParent = value;
         _tiedToStageSize = false;
         if(this._parent != null)
         {
            this._parent.removeEventListener(ResizeEvent.RESIZE,this.handleParentResize,false);
            if(this._tiedToParent)
            {
               this._parent.addEventListener(ResizeEvent.RESIZE,this.handleParentResize,false,0,true);
            }
         }
      }
      
      [Inspectable(defaultValue="true")]
      public function get hidden() : Boolean
      {
         return this._hidden;
      }
      
      public function set hidden(value:Boolean) : void
      {
         _hidden = value;
         visible = !this._hidden;
      }
      
      override public function get width() : Number
      {
         return super.width;
      }
      
      override public function set width(value:Number) : void
      {
         super.width = value;
         if(value > 0)
         {
            this.invalidate();
         }
      }
      
      override public function get height() : Number
      {
         return super.height;
      }
      
      override public function set height(value:Number) : void
      {
         super.height = value;
         if(value > 0)
         {
            this.invalidate();
         }
      }
      
      override public function get scaleX() : Number
      {
         return super.scaleX;
      }
      
      override public function set scaleX(value:Number) : void
      {
         super.scaleX = value;
         this.invalidate();
      }
      
      override public function get scaleY() : Number
      {
         return super.scaleY;
      }
      
      override public function set scaleY(value:Number) : void
      {
         super.scaleY = value;
         this.invalidate();
      }
      
      public function reflow() : void
      {
         if(this.beforeReflow is Function)
         {
            _tm2 = this.beforeReflow;
            beforeReflow = null;
            _tm2();
         }
         var i:uint = 0;
         var _tm2:Function;
         var spr:Sprite;
         var ld:LayoutData;
         var alignH:String;
         var alignV:String;
         var offsetH:Number;
         var offsetV:Number;
         var relativeH:String;
         var relHObject:Sprite;
         var relativeV:String;
         var relVObject:Sprite;
         var _tm:Function;
         while(i < this._managedSprites.length)
         {
            spr = this._managedSprites[i];
            ld = spr["layoutData"] as LayoutData;
            alignH = ld.alignH;
            alignV = ld.alignV;
            offsetH = this._tiedToStageSize && ld.offsetHashH[this._aspectRatio] != undefined ? (ld.offsetHashH[this._aspectRatio]) : ld.offsetH;
            offsetV = this._tiedToStageSize && ld.offsetHashV[this._aspectRatio] != undefined ? (ld.offsetHashV[this._aspectRatio]) : ld.offsetV;
            relativeH = ld.relativeToH;
            relHObject = relativeH != null ? this._parent.getChildByName(relativeH) as Sprite : null;
            relativeV = ld.relativeToV;
            relVObject = relativeV != null ? this._parent.getChildByName(relativeV) as Sprite : null;
            if(alignH != LayoutMode.ALIGN_NONE)
            {
               if(alignH == LayoutMode.ALIGN_LEFT)
               {
                  if(relHObject == null)
                  {
                     spr.x = this.rect.x + offsetH;
                  }
                  else
                  {
                     spr.x = relHObject.x - spr.width + offsetH;
                  }
               }
               else if(alignH == LayoutMode.ALIGN_RIGHT)
               {
                  if(relHObject == null)
                  {
                     spr.x = this.rect.width - spr.width + offsetH;
                     if(this._tiedToStageSize && (stage.align == STAGE_ALIGN_CENTER || stage.align == StageAlign.TOP || stage.align == StageAlign.BOTTOM))
                     {
                        spr.x += this.rect.x;
                     }
                  }
                  else
                  {
                     spr.x = relHObject.x + relHObject.width + offsetH;
                  }
               }
               else if(alignH == LayoutMode.ALIGN_CENTER)
               {
                  spr.x = (this.rect.width - this.rect.x) / 2 - spr.width / 2 + offsetH;
               }
            }
            if(alignV != LayoutMode.ALIGN_NONE)
            {
               if(alignV == LayoutMode.ALIGN_TOP)
               {
                  if(relVObject == null)
                  {
                     spr.y = this.rect.y + offsetV;
                  }
                  else
                  {
                     spr.y = relVObject.y - spr.height + offsetV;
                  }
               }
               else if(alignV == LayoutMode.ALIGN_BOTTOM)
               {
                  if(relVObject == null)
                  {
                     spr.y = this.rect.height - spr.height + offsetV;
                     if(this._tiedToStageSize && (stage.align == STAGE_ALIGN_CENTER || stage.align == StageAlign.TOP || stage.align == StageAlign.BOTTOM))
                     {
                        spr.y += this.rect.y;
                     }
                  }
                  else
                  {
                     spr.y = relVObject.y + relVObject.height + offsetV;
                  }
               }
               else if(alignV == LayoutMode.ALIGN_CENTER)
               {
                  spr.y = (this.rect.height - this.rect.y) / 2 - spr.height / 2 + offsetV;
               }
            }
            i++;
         }
         if(this.afterReflow is Function)
         {
            _tm = this.afterReflow;
            afterReflow = null;
            _tm();
         }
      }
      
      public function reset() : void
      {
         if(stage == null)
         {
            return;
         }
         _managedSprites = new Vector.<Sprite>();
         this.configUI();
      }
      
      public function resortManagedSprites() : void
      {
         var list:Vector.<Sprite> = new Vector.<Sprite>();
         while(this._managedSprites.length >= 1)
         {
            this.insertIntoSortedVector(this._managedSprites.pop(),list);
         }
         _managedSprites = list;
      }
      
      protected function invalidate() : void
      {
         addEventListener(Event.RENDER,this.handleStageInvalidation,false,0,true);
         if(stage)
         {
            stage.invalidate();
         }
      }
      
      protected function handleStageInvalidation(e:Event) : void
      {
         removeEventListener(Event.RENDER,this.handleStageInvalidation,false);
         this._rect.x = x;
         this._rect.y = y;
         this._rect.width = this.width;
         this._rect.height = this.height;
         this.reflow();
      }
      
      protected function handleAddedToStage(e:Event) : void
      {
         this.configUI();
      }
      
      protected function configUI() : void
      {
         removeEventListener(Event.ADDED_TO_STAGE,this.handleAddedToStage,false);
         _parent = parent as MovieClip;
         visible = !this._hidden;
         if(this.width > 0 && this.height > 0)
         {
            _rect = new Rectangle(x,y,this.width,this.height);
         }
         if(this._rect == null)
         {
            if(this._tiedToStageSize)
            {
               _rect = Extensions.enabled ? Extensions.visibleRect : new Rectangle(0,0,stage.stageWidth,stage.stageHeight);
            }
            else
            {
               _rect = new Rectangle(0,0,this._parent.width,this._parent.height);
               _tiedToParent = true;
            }
         }
         addEventListener(Event.ENTER_FRAME,this.handleFirstEnterFrame,false,0,true);
         this._parent.addEventListener(Event.ADDED_TO_STAGE,this.handleSpriteAddedToParent,false,0,true);
         this._parent.addEventListener(Event.REMOVED_FROM_STAGE,this.handleSpriteRemovedFromParent,false,0,true);
         if(this._tiedToStageSize)
         {
            stage.addEventListener(Event.RESIZE,this.handleStageResize,false,0,true);
         }
         else if(this._tiedToParent)
         {
            this._parent.addEventListener(ResizeEvent.RESIZE,this.handleParentResize,false,0,true);
         }
      }
      
      protected function handleFirstEnterFrame(e:Event) : void
      {
         removeEventListener(Event.ENTER_FRAME,this.handleFirstEnterFrame,false);
         this.analyzeSpritesOnStage();
      }
      
      protected function analyzeSpritesOnStage() : void
      {
         var i:uint = 0;
         var spr:Sprite;
         while(i < this._parent.numChildren)
         {
            spr = this._parent.getChildAt(i) as Sprite;
            if(spr != this)
            {
               this.evaluateLayoutForSprite(spr);
            }
            i++;
         }
         this.reflow();
      }
      
      protected function handleSpriteAddedToParent(e:Event) : void
      {
         var spr:Sprite = e.target as Sprite;
         if(spr.parent != this._parent)
         {
            return;
         }
         this.evaluateLayoutForSprite(spr);
         this.reflow();
      }
      
      protected function handleSpriteRemovedFromParent(e:Event) : void
      {
         var spr:Sprite = e.target as Sprite;
         if(spr.parent != this._parent)
         {
            return;
         }
         for(var i:uint = 0; i < this._managedSprites.length; i++)
         {
            if(this._managedSprites[i] == spr)
            {
               this._managedSprites.splice(i,1);
            }
         }
      }
      
      protected function handleParentResize(e:Event) : void
      {
         this._rect.width = this._parent.width;
         this._rect.height = this._parent.height;
         this.reflow();
      }
      
      protected function handleStageResize(e:Event) : void
      {
         this.updateAspectRatio();
         this.reflow();
      }
      
      protected function updateAspectRatio() : void
      {
         var ar:Number;
         if(Extensions.enabled && this._scaleFactor == 1)
         {
            _rect = Extensions.visibleRect;
            ar = this.rect.width / this.rect.height;
            switch(ar)
            {
               case 4 / 3:
                  _aspectRatio = LayoutData.ASPECT_RATIO_4_3;
                  break;
               case 16 / 9:
                  _aspectRatio = LayoutData.ASPECT_RATIO_16_9;
                  break;
               case 16 / 10:
                  _aspectRatio = LayoutData.ASPECT_RATIO_16_10;
               break;
               default:
                  break;
            }
         }
         else
         {
            this._rect.width = stage.stageWidth / this._scaleFactor;
            this._rect.height = stage.stageHeight / this._scaleFactor;
            trace("set_layout_rect",this._scaleFactor,this._rect.width,this._rect.height);
         }
      }
      
      public function setScaleFactor(value:Number) : void
      {
         trace("setScaleFactor",value);
         _scaleFactor = value;
         this.handleStageResize(null);
      }
      
      protected function evaluateLayoutForSprite(spr:Sprite) : void
      {
         if(spr == null)
         {
            return;
         }
         var lData:LayoutData = null;
         lData = spr["layoutData"];
         if(lData != null && (lData.layoutIdentifier == null || lData.layoutIdentifier == this.identifier))
         {
            this.insertIntoSortedVector(spr,this._managedSprites);
            this.calculateOffsets(spr);
         }
      }
      
      protected function insertIntoSortedVector(spr:Sprite, list:Vector.<Sprite>) : void
      {
         var lIndex:int = (spr["layoutData"] as LayoutData).layoutIndex;
         var inserted:Boolean;
         var i:uint;
         var ld:LayoutData;
         if(list.length == 0 || lIndex == -1)
         {
            list.push(spr);
         }
         else
         {
            inserted = false;
            i = 0;
            while(i < list.length && !inserted)
            {
               ld = list[i]["layoutData"] as LayoutData;
               if(lIndex >= 0 && lIndex <= ld.layoutIndex)
               {
                  list.splice(i,0,spr);
                  inserted = true;
               }
               i++;
            }
            if(!inserted)
            {
               list.push(spr);
            }
         }
      }
      
      protected function calculateOffsets(spr:Sprite) : void
      {
         var ld:LayoutData = spr["layoutData"] as LayoutData;
         var alignH:String = ld.alignH;
         var alignV:String = ld.alignV;
         var relativeH:String = ld.relativeToH;
         var relHObject:Sprite = relativeH != null ? this._parent.getChildByName(relativeH) as Sprite : null;
         var relativeV:String = ld.relativeToV;
         var relVObject:Sprite = relativeV != null ? this._parent.getChildByName(relativeV) as Sprite : null;
         if(ld.offsetH == -1)
         {
            if(alignH != LayoutMode.ALIGN_NONE)
            {
               if(alignH == LayoutMode.ALIGN_LEFT)
               {
                  if(relHObject == null)
                  {
                     ld.offsetH = spr.x - this.rect.x;
                  }
                  else
                  {
                     ld.offsetH = spr.x - (relHObject.x + relHObject.width);
                  }
               }
               else if(alignH == LayoutMode.ALIGN_RIGHT)
               {
                  if(relHObject == null)
                  {
                     ld.offsetH = spr.x + spr.width - this.rect.width;
                  }
                  else
                  {
                     ld.offsetH = spr.x - relHObject.x;
                  }
               }
            }
         }
         if(ld.offsetV == -1)
         {
            if(alignV != LayoutMode.ALIGN_NONE)
            {
               if(alignV == LayoutMode.ALIGN_TOP)
               {
                  if(relVObject == null)
                  {
                     ld.offsetV = spr.y - this.rect.y;
                  }
                  else
                  {
                     ld.offsetV = spr.y - (relVObject.y + relVObject.height);
                  }
               }
               else if(alignV == LayoutMode.ALIGN_BOTTOM)
               {
                  if(relVObject == null)
                  {
                     ld.offsetV = spr.y + spr.height - this.rect.height;
                  }
                  else
                  {
                     ld.offsetV = spr.y - relVObject.y;
                  }
               }
            }
         }
      }
   }
}

