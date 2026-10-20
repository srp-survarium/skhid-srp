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
      
      public function set rect(param1:Rectangle) : void
      {
         _rect = param1;
      }
      
      [Inspectable(defaultValue="false")]
      public function get tiedToStageSize() : Boolean
      {
         return this._tiedToStageSize;
      }
      
      public function set tiedToStageSize(param1:Boolean) : void
      {
         if(param1 == this._tiedToStageSize)
         {
            return;
         }
         _tiedToStageSize = param1;
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
      
      public function set tiedToParent(param1:Boolean) : void
      {
         if(param1 == this._tiedToParent)
         {
            return;
         }
         _tiedToParent = param1;
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
      
      public function set hidden(param1:Boolean) : void
      {
         _hidden = param1;
         visible = !this._hidden;
      }
      
      override public function get width() : Number
      {
         return super.width;
      }
      
      override public function set width(param1:Number) : void
      {
         super.width = param1;
         if(param1 > 0)
         {
            this.invalidate();
         }
      }
      
      override public function get height() : Number
      {
         return super.height;
      }
      
      override public function set height(param1:Number) : void
      {
         super.height = param1;
         if(param1 > 0)
         {
            this.invalidate();
         }
      }
      
      override public function get scaleX() : Number
      {
         return super.scaleX;
      }
      
      override public function set scaleX(param1:Number) : void
      {
         super.scaleX = param1;
         this.invalidate();
      }
      
      override public function get scaleY() : Number
      {
         return super.scaleY;
      }
      
      override public function set scaleY(param1:Number) : void
      {
         super.scaleY = param1;
         this.invalidate();
      }
      
      public function reflow() : void
      {
         if(this.beforeReflow is Function)
         {
            _loc2_ = this.beforeReflow;
            beforeReflow = null;
            _loc2_();
         }
         var _loc1_:uint = 0;
         var _loc2_:Function;
         var _loc3_:Sprite;
         var _loc4_:LayoutData;
         var _loc5_:String;
         var _loc6_:String;
         var _loc7_:Number;
         var _loc8_:Number;
         var _loc9_:String;
         var _loc10_:Sprite;
         var _loc11_:String;
         var _loc12_:Sprite;
         var _loc13_:Function;
         while(_loc1_ < this._managedSprites.length)
         {
            _loc3_ = this._managedSprites[_loc1_];
            _loc4_ = _loc3_["layoutData"] as LayoutData;
            _loc5_ = _loc4_.alignH;
            _loc6_ = _loc4_.alignV;
            _loc7_ = this._tiedToStageSize && _loc4_.offsetHashH[this._aspectRatio] != undefined ? (_loc4_.offsetHashH[this._aspectRatio]) : _loc4_.offsetH;
            _loc8_ = this._tiedToStageSize && _loc4_.offsetHashV[this._aspectRatio] != undefined ? (_loc4_.offsetHashV[this._aspectRatio]) : _loc4_.offsetV;
            _loc9_ = _loc4_.relativeToH;
            _loc10_ = _loc9_ != null ? this._parent.getChildByName(_loc9_) as Sprite : null;
            _loc11_ = _loc4_.relativeToV;
            _loc12_ = _loc11_ != null ? this._parent.getChildByName(_loc11_) as Sprite : null;
            if(_loc5_ != LayoutMode.ALIGN_NONE)
            {
               if(_loc5_ == LayoutMode.ALIGN_LEFT)
               {
                  if(_loc10_ == null)
                  {
                     _loc3_.x = this.rect.x + _loc7_;
                  }
                  else
                  {
                     _loc3_.x = _loc10_.x - _loc3_.width + _loc7_;
                  }
               }
               else if(_loc5_ == LayoutMode.ALIGN_RIGHT)
               {
                  if(_loc10_ == null)
                  {
                     _loc3_.x = this.rect.width - _loc3_.width + _loc7_;
                     if(this._tiedToStageSize && (stage.align == STAGE_ALIGN_CENTER || stage.align == StageAlign.TOP || stage.align == StageAlign.BOTTOM))
                     {
                        _loc3_.x += this.rect.x;
                     }
                  }
                  else
                  {
                     _loc3_.x = _loc10_.x + _loc10_.width + _loc7_;
                  }
               }
               else if(_loc5_ == LayoutMode.ALIGN_CENTER)
               {
                  _loc3_.x = (this.rect.width - this.rect.x) / 2 - _loc3_.width / 2 + _loc7_;
               }
            }
            if(_loc6_ != LayoutMode.ALIGN_NONE)
            {
               if(_loc6_ == LayoutMode.ALIGN_TOP)
               {
                  if(_loc12_ == null)
                  {
                     _loc3_.y = this.rect.y + _loc8_;
                  }
                  else
                  {
                     _loc3_.y = _loc12_.y - _loc3_.height + _loc8_;
                  }
               }
               else if(_loc6_ == LayoutMode.ALIGN_BOTTOM)
               {
                  if(_loc12_ == null)
                  {
                     _loc3_.y = this.rect.height - _loc3_.height + _loc8_;
                     if(this._tiedToStageSize && (stage.align == STAGE_ALIGN_CENTER || stage.align == StageAlign.TOP || stage.align == StageAlign.BOTTOM))
                     {
                        _loc3_.y += this.rect.y;
                     }
                  }
                  else
                  {
                     _loc3_.y = _loc12_.y + _loc12_.height + _loc8_;
                  }
               }
               else if(_loc6_ == LayoutMode.ALIGN_CENTER)
               {
                  _loc3_.y = (this.rect.height - this.rect.y) / 2 - _loc3_.height / 2 + _loc8_;
               }
            }
            _loc1_++;
         }
         if(this.afterReflow is Function)
         {
            _loc13_ = this.afterReflow;
            afterReflow = null;
            _loc13_();
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
         var _loc1_:Vector.<Sprite> = new Vector.<Sprite>();
         while(this._managedSprites.length >= 1)
         {
            this.insertIntoSortedVector(this._managedSprites.pop(),_loc1_);
         }
         _managedSprites = _loc1_;
      }
      
      protected function invalidate() : void
      {
         addEventListener(Event.RENDER,this.handleStageInvalidation,false,0,true);
         if(stage)
         {
            stage.invalidate();
         }
      }
      
      protected function handleStageInvalidation(param1:Event) : void
      {
         removeEventListener(Event.RENDER,this.handleStageInvalidation,false);
         this._rect.x = x;
         this._rect.y = y;
         this._rect.width = this.width;
         this._rect.height = this.height;
         this.reflow();
      }
      
      protected function handleAddedToStage(param1:Event) : void
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
      
      protected function handleFirstEnterFrame(param1:Event) : void
      {
         removeEventListener(Event.ENTER_FRAME,this.handleFirstEnterFrame,false);
         this.analyzeSpritesOnStage();
      }
      
      protected function analyzeSpritesOnStage() : void
      {
         var _loc1_:uint = 0;
         var _loc2_:Sprite;
         while(_loc1_ < this._parent.numChildren)
         {
            _loc2_ = this._parent.getChildAt(_loc1_) as Sprite;
            if(_loc2_ != this)
            {
               this.evaluateLayoutForSprite(_loc2_);
            }
            _loc1_++;
         }
         this.reflow();
      }
      
      protected function handleSpriteAddedToParent(param1:Event) : void
      {
         var _loc2_:Sprite = param1.target as Sprite;
         if(_loc2_.parent != this._parent)
         {
            return;
         }
         this.evaluateLayoutForSprite(_loc2_);
         this.reflow();
      }
      
      protected function handleSpriteRemovedFromParent(param1:Event) : void
      {
         var _loc2_:Sprite = param1.target as Sprite;
         if(_loc2_.parent != this._parent)
         {
            return;
         }
         var _loc3_:uint = 0;
         while(_loc3_ < this._managedSprites.length)
         {
            if(this._managedSprites[_loc3_] == _loc2_)
            {
               this._managedSprites.splice(_loc3_,1);
            }
            _loc3_++;
         }
      }
      
      protected function handleParentResize(param1:Event) : void
      {
         this._rect.width = this._parent.width;
         this._rect.height = this._parent.height;
         this.reflow();
      }
      
      protected function handleStageResize(param1:Event) : void
      {
         this.updateAspectRatio();
         this.reflow();
      }
      
      protected function updateAspectRatio() : void
      {
         var _loc1_:Number;
         if(Extensions.enabled && this._scaleFactor == 1)
         {
            _rect = Extensions.visibleRect;
            _loc1_ = this.rect.width / this.rect.height;
            switch(_loc1_)
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
      
      public function setScaleFactor(param1:Number) : void
      {
         trace("setScaleFactor",param1);
         _scaleFactor = param1;
         this.handleStageResize(null);
      }
      
      protected function evaluateLayoutForSprite(param1:Sprite) : void
      {
         if(param1 == null)
         {
            return;
         }
         var _loc2_:LayoutData = null;
         _loc2_ = param1["layoutData"];
         if(_loc2_ != null && (_loc2_.layoutIdentifier == null || _loc2_.layoutIdentifier == this.identifier))
         {
            this.insertIntoSortedVector(param1,this._managedSprites);
            this.calculateOffsets(param1);
         }
      }
      
      protected function insertIntoSortedVector(param1:Sprite, param2:Vector.<Sprite>) : void
      {
         var _loc3_:int = (param1["layoutData"] as LayoutData).layoutIndex;
         var _loc4_:Boolean;
         var _loc5_:uint;
         var _loc6_:LayoutData;
         if(param2.length == 0 || _loc3_ == -1)
         {
            param2.push(param1);
         }
         else
         {
            _loc4_ = false;
            _loc5_ = 0;
            while(_loc5_ < param2.length && !_loc4_)
            {
               _loc6_ = param2[_loc5_]["layoutData"] as LayoutData;
               if(_loc3_ >= 0 && _loc3_ <= _loc6_.layoutIndex)
               {
                  param2.splice(_loc5_,0,param1);
                  _loc4_ = true;
               }
               _loc5_++;
            }
            if(!_loc4_)
            {
               param2.push(param1);
            }
         }
      }
      
      protected function calculateOffsets(param1:Sprite) : void
      {
         var _loc2_:LayoutData = param1["layoutData"] as LayoutData;
         var _loc3_:String = _loc2_.alignH;
         var _loc4_:String = _loc2_.alignV;
         var _loc5_:String = _loc2_.relativeToH;
         var _loc6_:Sprite = _loc5_ != null ? this._parent.getChildByName(_loc5_) as Sprite : null;
         var _loc7_:String = _loc2_.relativeToV;
         var _loc8_:Sprite = _loc7_ != null ? this._parent.getChildByName(_loc7_) as Sprite : null;
         if(_loc2_.offsetH == -1)
         {
            if(_loc3_ != LayoutMode.ALIGN_NONE)
            {
               if(_loc3_ == LayoutMode.ALIGN_LEFT)
               {
                  if(_loc6_ == null)
                  {
                     _loc2_.offsetH = param1.x - this.rect.x;
                  }
                  else
                  {
                     _loc2_.offsetH = param1.x - (_loc6_.x + _loc6_.width);
                  }
               }
               else if(_loc3_ == LayoutMode.ALIGN_RIGHT)
               {
                  if(_loc6_ == null)
                  {
                     _loc2_.offsetH = param1.x + param1.width - this.rect.width;
                  }
                  else
                  {
                     _loc2_.offsetH = param1.x - _loc6_.x;
                  }
               }
            }
         }
         if(_loc2_.offsetV == -1)
         {
            if(_loc4_ != LayoutMode.ALIGN_NONE)
            {
               if(_loc4_ == LayoutMode.ALIGN_TOP)
               {
                  if(_loc8_ == null)
                  {
                     _loc2_.offsetV = param1.y - this.rect.y;
                  }
                  else
                  {
                     _loc2_.offsetV = param1.y - (_loc8_.y + _loc8_.height);
                  }
               }
               else if(_loc4_ == LayoutMode.ALIGN_BOTTOM)
               {
                  if(_loc8_ == null)
                  {
                     _loc2_.offsetV = param1.y + param1.height - this.rect.height;
                  }
                  else
                  {
                     _loc2_.offsetV = param1.y - _loc8_.y;
                  }
               }
            }
         }
      }
   }
}

